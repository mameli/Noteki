import type { EditorState, Line } from "@codemirror/state";

/** A part of the document, from `from` to `to`. */
export interface Span {
  from: number;
  to: number;
}

/** The lines from the one holding `from` to the one holding `to`. */
export function* linesOf(state: EditorState, from: number, to: number): Generator<Line> {
  for (let line = state.doc.lineAt(from); ; line = state.doc.line(line.number + 1)) {
    yield line;
    if (line.to >= to || line.number === state.doc.lines) return;
  }
}
