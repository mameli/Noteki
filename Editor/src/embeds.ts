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

/** The file an Embed or a Markdown image shows, if the app found one. */
export function embeddedFile(state: EditorState, node: SyntaxNodeRef): EmbeddedFile | undefined {
  const inner = innerOf(node, state.doc);
  return inner === null ? undefined : state.field(embedFiles, false)?.[inner];
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
    // The app serves the Vault's images under this scheme, and nothing else.
    const version = this.spec.file.version === undefined ? "" : `?v=${this.spec.file.version}`;
    image.src = "noteki-file://vault/" + this.spec.file.path.split("/").map(encodeURIComponent).join("/") + version;
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

function imageDecorations(state: EditorState): DecorationSet {
  const ranges: Range<Decoration>[] = imageWidgets(state).map((spec) =>
    Decoration.widget({ widget: new ImageWidget(spec), block: true, side: 1 }).range(spec.at),
  );
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
