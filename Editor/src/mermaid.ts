import { syntaxTree } from "@codemirror/language";
import { type EditorState, type Range, StateField } from "@codemirror/state";
import { Decoration, type DecorationSet, EditorView, WidgetType } from "@codemirror/view";
import { coloursChanged } from "./settings";

// Mermaid diagrams: a fenced code block of language `mermaid` is drawn as its diagram while the
// cursor is out of it, and its source shows with the diagram under it while the cursor is in it,
// as Obsidian does. The library is 3.5 MB: it is a file of its own next to the page
// (mermaid.min.js), loaded the first time a Note has a diagram, never otherwise.

/** A diagram's block: from its opening fence's line to its closing one, and its source. */
export interface MermaidBlock {
  from: number;
  to: number;
  source: string;
}

/** The Note's `mermaid` code blocks, closed ones only. */
export function mermaidBlocks(state: EditorState): MermaidBlock[] {
  const blocks: MermaidBlock[] = [];
  syntaxTree(state).iterate({
    enter(node) {
      if (node.name !== "FencedCode") return undefined;
      const info = node.node.getChild("CodeInfo");
      const marks = node.node.getChildren("CodeMark");
      if (!info || state.sliceDoc(info.from, info.to).trim().toLowerCase() !== "mermaid" || marks.length < 2) return false;
      const first = state.doc.lineAt(node.from);
      const last = state.doc.lineAt(node.to);
      const text = node.node.getChild("CodeText");
      blocks.push({ from: first.from, to: last.to, source: text ? state.sliceDoc(text.from, text.to) : "" });
      return false;
    },
  });
  return blocks;
}

/** Whether a cursor, or a selection's end, is in the block: its source shows. */
function entered(state: EditorState, block: MermaidBlock): boolean {
  return state.selection.ranges.some((range) => range.head >= block.from && range.head <= block.to);
}

interface Mermaid {
  initialize(config: Record<string, unknown>): void;
  render(id: string, source: string): Promise<{ svg: string }>;
}

let loading: Promise<Mermaid> | null = null;

/** The library, loaded once from the file next to the page. */
function loadMermaid(): Promise<Mermaid> {
  loading ??= new Promise((resolve, reject) => {
    const script = document.createElement("script");
    script.src = "mermaid.min.js";
    script.onload = () => resolve((window as unknown as { mermaid: Mermaid }).mermaid);
    script.onerror = () => reject(new Error("mermaid.min.js is missing from the app"));
    document.head.append(script);
  });
  return loading;
}

let rendered = 0;

/** Draws `source` in the theme's colours: Mermaid's own `base` theme, given the page's. */
export async function render(source: string): Promise<string> {
  const mermaid = await loadMermaid();
  const colours = getComputedStyle(document.documentElement);
  const value = (name: string) => colours.getPropertyValue(name).trim();
  mermaid.initialize({
    startOnLoad: false,
    // Its default: no script or click handler of the diagram runs.
    securityLevel: "strict",
    theme: "base",
    fontFamily: value("--text-font") || "-apple-system, sans-serif",
    themeVariables: {
      background: value("--background"),
      primaryColor: value("--panel"),
      primaryTextColor: value("--text"),
      primaryBorderColor: value("--separator"),
      secondaryColor: value("--code-background"),
      tertiaryColor: value("--background"),
      lineColor: value("--syntax"),
      textColor: value("--text"),
      mainBkg: value("--panel"),
      nodeBorder: `rgb(${value("--accent-rgb")})`,
      clusterBkg: value("--code-background"),
      edgeLabelBackground: value("--background"),
    },
  });
  rendered += 1;
  return (await mermaid.render(`noteki-mermaid-${rendered}`, source)).svg;
}

class MermaidWidget extends WidgetType {
  private readonly redraws = new WeakMap<HTMLElement, { scheme: MediaQueryList; draw: () => void }>();

  constructor(
    readonly source: string,
    /** Where a click puts the cursor, to edit the source: its first line of code. */
    readonly at: number,
  ) {
    super();
  }

  eq(other: MermaidWidget) {
    return other.source === this.source && other.at === this.at;
  }

  toDOM(view: EditorView) {
    const box = document.createElement("div");
    box.className = "cm-mermaid";
    const draw = () => {
      render(this.source).then(
        (svg) => {
          box.classList.remove("cm-mermaid-error");
          box.innerHTML = svg;
          view.requestMeasure();
        },
        (error: unknown) => {
          box.classList.add("cm-mermaid-error");
          box.textContent = `Mermaid: ${error instanceof Error ? error.message : String(error)}`;
          view.requestMeasure();
        },
      );
    };
    draw();
    const scheme = matchMedia("(prefers-color-scheme: dark)");
    scheme.addEventListener("change", draw);
    window.addEventListener(coloursChanged, draw);
    this.redraws.set(box, { scheme, draw });
    box.addEventListener("mousedown", (event) => {
      event.preventDefault();
      view.dispatch({ selection: { anchor: this.at } });
      view.focus();
    });
    return box;
  }

  destroy(dom: HTMLElement) {
    const redraw = this.redraws.get(dom);
    redraw?.scheme.removeEventListener("change", redraw.draw);
    if (redraw) window.removeEventListener(coloursChanged, redraw.draw);
  }

  ignoreEvent() {
    return false;
  }
}

function mermaidDecorations(state: EditorState): DecorationSet {
  const ranges: Range<Decoration>[] = [];
  for (const block of mermaidBlocks(state)) {
    const at = Math.min(state.doc.lineAt(block.from).to + 1, block.to);
    const widget = new MermaidWidget(block.source, at);
    if (entered(state, block)) ranges.push(Decoration.widget({ widget, block: true, side: 1 }).range(block.to));
    else ranges.push(Decoration.replace({ widget, block: true }).range(block.from, block.to));
  }
  return Decoration.set(ranges, true);
}

/** The diagrams: block widgets, and replacements across lines, come from the state. */
export const mermaidDiagrams = [
  StateField.define<DecorationSet>({
    create: mermaidDecorations,
    update(value, transaction) {
      const changed = transaction.docChanged || transaction.selection || syntaxTree(transaction.startState) !== syntaxTree(transaction.state);
      return changed ? mermaidDecorations(transaction.state) : value;
    },
    provide: (field) => EditorView.decorations.from(field),
  }),
  EditorView.baseTheme({
    ".cm-mermaid": { padding: "8px 0", display: "flex", justifyContent: "center", cursor: "text", minHeight: "2em" },
    ".cm-mermaid svg": { maxWidth: "100%", height: "auto" },
    ".cm-mermaid-error": { color: "var(--error)", fontFamily: "var(--mono)", fontSize: "0.85em", justifyContent: "flex-start" },
  }),
];
