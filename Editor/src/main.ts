import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { tags } from "@lezer/highlight";
import { type EditorCommand, send } from "./bridge";
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

function initialState(text: string) {
  return EditorState.create({
    doc: text,
    extensions: [
      markdown({ base: markdownLanguage, extensions: [frontmatter] }),
      noteTheme,
      syntaxHighlighting(sourceHighlight),
      EditorView.lineWrapping,
      EditorState.readOnly.of(true),
      EditorView.editable.of(false),
    ],
  });
}

const view = new EditorView({
  state: initialState(""),
  parent: document.getElementById("editor")!,
});

window.noteki = {
  receive(command: EditorCommand) {
    switch (command.type) {
      case "load":
        view.setState(initialState(command.text));
        view.scrollDOM.scrollTop = 0;
        break;
    }
  },
};

send({ type: "ready" });
