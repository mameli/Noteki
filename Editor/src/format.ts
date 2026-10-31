import { EditorSelection, type EditorState, type SelectionRange, type TransactionSpec } from "@codemirror/state";

// The Format menu (⌘B, ⌘I, ⌘1…): Markdown written around the selection or on its lines, as
// Obsidian writes it. Every action is one step of undo, and works on every selection.

/** What the Format menu asks for; a heading's level comes with it. */
export type Formatting =
  | "bold"
  | "italic"
  | "strikethrough"
  | "code"
  | "comment"
  | "link"
  | "heading1"
  | "heading2"
  | "heading3"
  | "heading4"
  | "heading5"
  | "heading6"
  | "task"
  | "quote"
  | "bulletList"
  | "numberedList";

const marks: Partial<Record<Formatting, string>> = {
  bold: "**",
  italic: "*",
  strikethrough: "~~",
  code: "`",
  comment: "%%",
};

/** How many `char` are written in a row going from `pos` backward (-1) or forward (1). */
function run(state: EditorState, pos: number, char: string, direction: 1 | -1): number {
  let count = 0;
  for (let at = direction === 1 ? pos : pos - 1; at >= 0 && at < state.doc.length && state.sliceDoc(at, at + 1) === char; at += direction) count++;
  return count;
}

/**
 * Whether `count` marks in a row make `mark`: `*` alone or with `**` (`***both***`), `**` alone or
 * with `*`; the others as written.
 */
function makes(count: number, mark: string): boolean {
  if (mark === "*") return count === 1 || count === 3;
  if (mark === "**") return count === 2 || count === 3;
  return count >= mark.length;
}

/** The text a mark goes around when nothing is selected: the word at the cursor, else the cursor. */
function target(state: EditorState, range: SelectionRange, mark: string): { from: number; to: number } {
  if (!range.empty) return range;
  // A Comment of nothing selected is the whole line, as Obsidian's.
  if (mark === "%%") {
    const line = state.doc.lineAt(range.head);
    return line.length > 0 ? line : range;
  }
  return state.wordAt(range.head) ?? range;
}

/** `mark` around each selection, or taken away where it is already written around it. */
export function toggleMark(state: EditorState, mark: string): TransactionSpec {
  const char = mark[0];
  return state.changeByRange((range) => {
    const { from, to } = target(state, range, mark);
    // Written around it: `**word**` with `word` selected.
    if (makes(run(state, from, char, -1), mark) && makes(run(state, to, char, 1), mark) && from - mark.length >= 0) {
      return {
        changes: [
          { from: from - mark.length, to: from },
          { from: to, to: to + mark.length },
        ],
        range: EditorSelection.range(range.anchor - mark.length, range.head - mark.length),
      };
    }
    // Selected with it: `**word**` all selected.
    const text = state.sliceDoc(from, to);
    if (to - from >= 2 * mark.length && text.startsWith(mark) && text.endsWith(mark)) {
      return {
        changes: { from, to, insert: text.slice(mark.length, -mark.length) },
        range: EditorSelection.range(from, to - 2 * mark.length),
      };
    }
    const changes = [
      { from, insert: mark },
      { from: to, insert: mark },
    ];
    // Nothing selected and no word: the cursor between the marks, to type.
    if (from === to) return { changes, range: EditorSelection.cursor(from + mark.length) };
    return { changes, range: EditorSelection.range(range.anchor + mark.length, range.head + mark.length) };
  });
}

/**
 * `[text](url)` of the selection or the word at the cursor: with a web address (the one on the
 * pasteboard) the cursor goes after the link, otherwise between its parentheses to type one.
 */
export function insertLink(state: EditorState, url?: string): TransactionSpec {
  return state.changeByRange((range) => {
    const { from, to } = range.empty ? (state.wordAt(range.head) ?? range) : range;
    const text = state.sliceDoc(from, to);
    const insert = `[${text}](${url ?? ""})`;
    // No text: between the brackets, to type it.
    const cursor = text === "" ? from + 1 : url ? from + insert.length : from + insert.length - 1;
    return { changes: { from, to, insert }, range: EditorSelection.cursor(cursor) };
  });
}

/** The lines the selections touch, once each. */
function selectedLines(state: EditorState) {
  const lines = new Map<number, { from: number; text: string }>();
  for (const range of state.selection.ranges) {
    for (let at = state.doc.lineAt(range.from).number; at <= state.doc.lineAt(range.to).number; at++) {
      const line = state.doc.line(at);
      lines.set(at, { from: line.from, text: line.text });
    }
  }
  return [...lines.values()];
}

/** The lines as headings of `level`; lines already of that level become text again. */
export function toggleHeading(state: EditorState, level: number): TransactionSpec {
  const lines = selectedLines(state);
  const written = (text: string) => /^#{1,6}(?=\s|$)/.exec(text)?.[0].length ?? 0;
  // All of that level already: ⌘2 again takes the heading away.
  const off = lines.every((line) => written(line.text) === level);
  const changes = lines.map((line) => {
    const old = /^#{1,6}(\s+|$)/.exec(line.text)?.[0].length ?? 0;
    return { from: line.from, to: line.from + old, insert: off ? "" : "#".repeat(level) + " " };
  });
  return { changes, scrollIntoView: true, userEvent: "input" };
}

/**
 * The lines as tasks: a task is ticked or cleared, a list item gets its checkbox, other text
 * becomes `- [ ] text`, its indentation kept.
 */
export function toggleTask(state: EditorState): TransactionSpec {
  const changes = selectedLines(state).flatMap((line) => {
    const task = /^(\s*(?:[-*+]|\d+[.)]) \[)([ xX])\]/.exec(line.text);
    if (task) {
      const at = line.from + task[1].length;
      return [{ from: at, to: at + 1, insert: task[2] === " " ? "x" : " " }];
    }
    const item = /^\s*(?:[-*+]|\d+[.)]) /.exec(line.text);
    if (item) return [{ from: line.from + item[0].length, insert: "[ ] " }];
    const indent = /^\s*/.exec(line.text)?.[0].length ?? 0;
    return [{ from: line.from + indent, insert: "- [ ] " }];
  });
  return { changes, scrollIntoView: true, userEvent: "input" };
}

/**
 * The Format menu's shortcuts, for the keys the web view takes before the menu sees them (⌘I,
 * ⌘↩ and ⌘/ are WebKit's or CodeMirror's own): each asks the app, which answers as the menu does,
 * a link with the pasteboard's address.
 */
export const formatKeys: { key: string; action: Formatting }[] = [
  { key: "Mod-b", action: "bold" },
  { key: "Mod-i", action: "italic" },
  { key: "Mod-Shift-x", action: "strikethrough" },
  { key: "Ctrl-`", action: "code" },
  { key: "Mod-/", action: "comment" },
  { key: "Mod-k", action: "link" },
  { key: "Mod-Enter", action: "task" },
  ...([1, 2, 3, 4, 5, 6] as const).map((level) => ({ key: `Mod-${level}`, action: `heading${level}` as Formatting })),
];

const listMarker = /^(\s*)(?:[-*+]|\d+[.)]) (?:\[[ xX]\] )?/;

/**
 * The lines as a list of `kind`, numbered from 1; a list item of another kind, a task included,
 * changes kind. Lines all of that kind already become text again.
 */
export function toggleList(state: EditorState, kind: "bullet" | "numbered"): TransactionSpec {
  const lines = selectedLines(state).filter((line) => line.text.trim() !== "" || state.selection.main.empty);
  const isKind = (text: string) => (kind === "bullet" ? /^\s*[-*+] (?!\[[ xX]\] )/ : /^\s*\d+[.)] (?!\[[ xX]\] )/).test(text);
  const off = lines.every((line) => isKind(line.text));
  const changes = lines.map((line, index) => {
    const marker = listMarker.exec(line.text);
    const indent = marker ? marker[1].length : (/^\s*/.exec(line.text)?.[0].length ?? 0);
    const to = line.from + (marker ? marker[0].length : indent);
    const insert = off ? "" : kind === "bullet" ? "- " : `${index + 1}. `;
    return { from: line.from + indent, to, insert };
  });
  return { changes, scrollIntoView: true, userEvent: "input" };
}

/** The lines as a quote, `> ` before each; lines all quoted already lose it. */
export function toggleQuote(state: EditorState): TransactionSpec {
  const lines = selectedLines(state);
  const off = lines.every((line) => /^>\s?/.test(line.text));
  const changes = lines.map((line) => {
    if (!off) return { from: line.from, insert: "> " };
    return { from: line.from, to: line.from + (/^>\s?/.exec(line.text)?.[0].length ?? 0) };
  });
  return { changes, scrollIntoView: true, userEvent: "input" };
}

/** The Format menu's action, as a transaction; a link with the pasteboard's web address, if any. */
export function formatting(state: EditorState, action: Formatting, url?: string): TransactionSpec {
  const mark = marks[action];
  if (mark) return { ...toggleMark(state, mark), scrollIntoView: true, userEvent: "input" };
  if (action === "link") return { ...insertLink(state, url), scrollIntoView: true, userEvent: "input" };
  if (action === "task") return toggleTask(state);
  if (action === "quote") return toggleQuote(state);
  if (action === "bulletList") return toggleList(state, "bullet");
  if (action === "numberedList") return toggleList(state, "numbered");
  return toggleHeading(state, Number(action.slice("heading".length)));
}
