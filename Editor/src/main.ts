import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { EditorState } from "@codemirror/state";
import { EditorView, keymap } from "@codemirror/view";
import { tags } from "@lezer/highlight";
import { type EditorCommand, type EditorMessage, send } from "./bridge";
import { frontmatter } from "./frontmatter";

// Colours come from the CSS variables in editor.css, so light and dark follow the system.
const sourceHighlight = HighlightStyle.define([
  { tag: tags.heading1, fontSize: "1.6em", fontWeight: "700" },
  { tag: tags.heading2, fontSize: "1.35em", fontWeight: "700" },
  { tag: tags.heading3, fontSize: "1.15em", fontWeight: "700" },
  { tag: [tags.heading4, tags.heading5, tags.heading6], fontWeight: "700" },
  { tag: tags.strong, fontWeight: "700" },
  { tag: tags.emphasis, fontStyle: "italic" },
  { tag: tags.strikethrough, textDecoration: "line-through" },
  { tag: tags.link, color: "var(--link)" },
  { tag: tags.url, color: "var(--link)" },
  { tag: tags.monospace, fontFamily: "var(--mono)" },
  { tag: tags.quote, color: "var(--quote)" },
  { tag: [tags.processingInstruction, tags.meta, tags.contentSeparator], color: "var(--syntax)" },
  { tag: tags.comment, color: "var(--syntax)" },
]);

// Set here rather than in editor.css: CodeMirror's base theme would win over a stylesheet.
const noteTheme = EditorView.theme({
  ".cm-scroller": { fontFamily: "-apple-system, BlinkMacSystemFont, sans-serif", fontSize: "15px", lineHeight: "1.6" },
  ".cm-content": { maxWidth: "var(--line-width)", margin: "0 auto", padding: "2rem 1.5rem 40vh" },
});

/** How often, at most, the text is reported while the user types. The app saves after it. */
const reportInterval = 250;

/** The Note shown, by its path; null before the first one. */
let noteID: string | null = null;
let unreported = false;
let revision = 0;
let reportTimer: ReturnType<typeof setTimeout> | undefined;

function report(): EditorMessage | null {
  clearTimeout(reportTimer);
  reportTimer = undefined;
  if (noteID === null || !unreported) return null;
  unreported = false;
  revision += 1;
  return { type: "changed", noteID, revision, text: view.state.doc.toString() };
}

const reportChanges = EditorView.updateListener.of((update) => {
  if (!update.docChanged) return;
  unreported = true;
  reportTimer ??= setTimeout(() => {
    const message = report();
    if (message) send(message);
  }, reportInterval);
});

function initialState(text: string, editable: boolean) {
  return EditorState.create({
    doc: text,
    extensions: [
      markdown({ base: markdownLanguage, extensions: [frontmatter] }),
      noteTheme,
      syntaxHighlighting(sourceHighlight),
      EditorView.lineWrapping,
      history(),
      keymap.of([...defaultKeymap, ...historyKeymap]),
      EditorView.editable.of(editable),
      reportChanges,
    ],
  });
}

const view = new EditorView({
  state: initialState("", false),
  parent: document.getElementById("editor")!,
});

window.noteki = {
  receive(command: EditorCommand) {
    switch (command.type) {
      case "load": {
        // The app flushes before switching Note: this only catches what slipped through.
        const pending = report();
        if (pending) send(pending);
        noteID = command.noteID;
        view.setState(initialState(command.text, true));
        view.scrollDOM.scrollTop = 0;
        return null;
      }
      case "clear": {
        const pending = report();
        if (pending) send(pending);
        noteID = null;
        view.setState(initialState("", false));
        return null;
      }
      case "flush": {
        const message = report();
        return message ? JSON.stringify(message) : null;
      }
    }
  },
};

send({ type: "ready" });
