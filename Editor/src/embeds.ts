import { syntaxTree } from "@codemirror/language";
import { type EditorState, type Range, StateEffect, StateField } from "@codemirror/state";
import { Decoration, type DecorationSet, EditorView, WidgetType } from "@codemirror/view";
import type { SyntaxNodeRef } from "@lezer/common";
import { innerOf } from "./wikilink";

// Embeds of images and PDFs (SPEC, Embeds and Attachments). The app says which file each Embed
// shows and an image's size; the editor draws the image under the Embed's line, always, with the
// room its size needs before it loads. Only the Embed's source hides while the cursor is away
// (livepreview.ts), so nothing moves when the cursor comes and goes.

/** The app's EmbeddedFile: what an Embed shows. */
export interface EmbeddedFile {
  path: string;
  kind: "image" | "pdf";
  width?: number;
  height?: number;
  /** When the file last changed: an image changed on disk is loaded again. */
  version?: number;
  /** An image on the web, `![](https://…)`: loaded from there, its size unknown until it arrives. */
  url?: string;
}

/** The app's answer: the files of the Note's Embeds, by what the app was asked (`innerOf`). */
export const setEmbeds = StateEffect.define<Record<string, EmbeddedFile>>();

export const embedFiles = StateField.define<Record<string, EmbeddedFile>>({
  create: () => ({}),
  update(value, transaction) {
    for (const effect of transaction.effects) if (effect.is(setEmbeds)) value = effect.value;
    return value;
  },
});

/** The file an Embed or a Markdown image shows, if the app found one, or the image on the web. */
export function embeddedFile(state: EditorState, node: SyntaxNodeRef): EmbeddedFile | undefined {
  const url = webImage(state, node);
  if (url) return { path: url, kind: "image", url };
  const inner = innerOf(node, state.doc);
  return inner === null ? undefined : state.field(embedFiles, false)?.[inner];
}

/** The web address of a Markdown image `![](https://…)`, the app is not asked about it. */
function webImage(state: EditorState, node: SyntaxNodeRef): string | null {
  if (node.name !== "Image") return null;
  const url = node.node.getChild("URL");
  if (!url) return null;
  let address = state.doc.sliceString(url.from, url.to);
  if (address.startsWith("<") && address.endsWith(">")) address = address.slice(1, -1);
  return /^https?:\/\//i.test(address) ? address : null;
}

/** Where an image is drawn: after the end of the Embed's line, and how wide if its Embed says. */
export interface ImageWidgetSpec {
  at: number;
  file: EmbeddedFile;
  /** `![[image.png|300]]`: 300 pixels wide, as in Obsidian. */
  width?: number;
}

/** The images of the Note's Embeds and Markdown images that the app found, in order. */
export function imageWidgets(state: EditorState): ImageWidgetSpec[] {
  const widgets: ImageWidgetSpec[] = [];
  syntaxTree(state).iterate({
    enter(node) {
      // A block under a table's row would break the table drawn in place (tables.ts).
      if (node.name === "Table") return false;
      if (node.name !== "Embed" && node.name !== "Image") return undefined;
      const file = embeddedFile(state, node);
      if (file?.kind === "image") {
        const size = node.name === "Embed" ? /\|\s*(\d+)(?:x\d+)?\s*$/.exec(state.doc.sliceString(node.from + 3, node.to - 2)) : null;
        widgets.push({ at: state.doc.lineAt(node.to).to, file, width: size ? Number(size[1]) : undefined });
      }
      return false;
    },
  });
  return widgets;
}

/**
 * An SVG written in the Note, `<svg …>…</svg>` as a paragraph or an HTML block of its own, as a
 * web clipper saves a page's diagrams: its source.
 */
export function svgSource(state: EditorState, node: SyntaxNodeRef): string | null {
  if (node.name !== "Paragraph" && node.name !== "HTMLBlock") return null;
  const text = state.doc.sliceString(node.from, node.to).trim();
  return /^<svg[\s>]/i.test(text) && /<\/svg>$/i.test(text) ? text : null;
}

/** The SVGs written in the Note, each drawn under its last line. */
export function svgWidgets(state: EditorState): { at: number; source: string }[] {
  const widgets: { at: number; source: string }[] = [];
  syntaxTree(state).iterate({
    enter(node) {
      if (node.name === "Table" || node.name === "FencedCode" || node.name === "CodeBlock") return false;
      const source = svgSource(state, node);
      if (source === null) return undefined;
      widgets.push({ at: state.doc.lineAt(node.to).to, source });
      return false;
    },
  });
  return widgets;
}

class ImageWidget extends WidgetType {
  constructor(readonly spec: ImageWidgetSpec) {
    super();
  }

  eq(other: ImageWidget) {
    const [a, b] = [this.spec, other.spec];
    return a.file.path === b.file.path && a.width === b.width && a.file.version === b.file.version && a.file.width === b.file.width && a.file.height === b.file.height;
  }

  toDOM() {
    const box = document.createElement("div");
    box.className = "cm-embed-image";
    const image = document.createElement("img");
    // The app serves the Vault's images under this scheme; an image on the web comes from there.
    const version = this.spec.file.version === undefined ? "" : `?v=${this.spec.file.version}`;
    image.src = this.spec.file.url ?? "noteki-file://vault/" + this.spec.file.path.split("/").map(encodeURIComponent).join("/") + version;
    image.alt = this.spec.file.path;
    image.draggable = false;
    const { width, height } = this.spec.file;
    // Its room is known before it loads: the text below does not move when it arrives.
    if (width && height) image.style.aspectRatio = `${width} / ${height}`;
    const wanted = this.spec.width ?? width;
    if (wanted) image.style.width = `${wanted}px`;
    box.append(image);
    return box;
  }

  /** Its height as drawn: at its width or the text's (46 rem), whichever is less. */
  get estimatedHeight() {
    const { width, height } = this.spec.file;
    if (!width || !height) return -1;
    const drawn = Math.min(this.spec.width ?? width, 736);
    return Math.round((drawn * height) / width) + 8;
  }

  ignoreEvent() {
    return false;
  }
}

/**
 * An SVG drawn as an image, never in the page: no script of it runs and its styles stay inside.
 * An image does not inherit the text's colour, so `currentColor` and `Canvas` are given the
 * editor's, and drawn again when the Mac switches between light and dark.
 */
const dark = "(prefers-color-scheme: dark)";

class SvgWidget extends WidgetType {
  private readonly redraws = new WeakMap<HTMLElement, { scheme: MediaQueryList; draw: () => void }>();

  constructor(readonly source: string) {
    super();
  }

  eq(other: SvgWidget) {
    return other.source === this.source;
  }

  toDOM(view: EditorView) {
    const box = document.createElement("div");
    box.className = "cm-embed-image";
    const image = document.createElement("img");
    image.alt = "SVG";
    image.draggable = false;
    box.append(image);
    const draw = () => this.draw(view, image);
    draw();
    const scheme = matchMedia(dark);
    scheme.addEventListener("change", draw);
    this.redraws.set(box, { scheme, draw });
    return box;
  }

  private draw(view: EditorView, image: HTMLImageElement) {
    // HTML's parser, as Obsidian's page reads it: lenient where XML's would refuse.
    const svg = new DOMParser().parseFromString(this.source, "text/html").querySelector("svg");
    if (!svg) return;
    const text = getComputedStyle(view.contentDOM).color;
    const background = getComputedStyle(document.documentElement).getPropertyValue("--background").trim();
    for (const element of [svg, ...svg.querySelectorAll("*")]) {
      for (const name of ["fill", "stroke", "style"]) {
        const value = element.getAttribute(name);
        if (value) element.setAttribute(name, value.replace(/\bCanvasText\b/g, text).replace(/\bCanvas\b/g, background));
      }
    }
    svg.style.color = text;
    const width = Number(svg.getAttribute("width"));
    const height = Number(svg.getAttribute("height"));
    if (width > 0) image.style.width = `${width}px`;
    if (width > 0 && height > 0) image.style.aspectRatio = `${width} / ${height}`;
    image.src = "data:image/svg+xml;charset=utf-8," + encodeURIComponent(new XMLSerializer().serializeToString(svg));
  }

  destroy(dom: HTMLElement) {
    const redraw = this.redraws.get(dom);
    redraw?.scheme.removeEventListener("change", redraw.draw);
  }

  ignoreEvent() {
    return false;
  }
}

function imageDecorations(state: EditorState): DecorationSet {
  const ranges: Range<Decoration>[] = [
    ...imageWidgets(state).map((spec) => Decoration.widget({ widget: new ImageWidget(spec), block: true, side: 1 }).range(spec.at)),
    ...svgWidgets(state).map((spec) => Decoration.widget({ widget: new SvgWidget(spec.source), block: true, side: 1 }).range(spec.at)),
  ];
  return Decoration.set(ranges, true);
}

/** The images drawn, a block under their Embed's line: block widgets come from the state. */
export const embedImages = [
  StateField.define<DecorationSet>({
    create: imageDecorations,
    update(value, transaction) {
      const changed =
        transaction.docChanged ||
        transaction.effects.some((effect) => effect.is(setEmbeds)) ||
        syntaxTree(transaction.startState) !== syntaxTree(transaction.state);
      return changed ? imageDecorations(transaction.state) : value;
    },
    provide: (field) => EditorView.decorations.from(field),
  }),
  EditorView.baseTheme({
    ".cm-embed-image": { padding: "4px 0" },
    ".cm-embed-image img": { display: "block", maxWidth: "100%", height: "auto", borderRadius: "4px" },
  }),
];
