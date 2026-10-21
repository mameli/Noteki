import { defaultKeymap, history, historyKeymap, redo, selectAll, undo } from "@codemirror/commands";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { findNext, findPrevious, openSearchPanel, search, searchKeymap } from "@codemirror/search";
import { Annotation, EditorState, Transaction } from "@codemirror/state";
import { EditorView, keymap } from "@codemirror/view";
import { tags } from "@lezer/highlight";
import { type EditorCommand, type EditorMessage, send } from "./bridge";
import { linkCompletion, receiveLinkCompletions } from "./completion";
import { frontmatter } from "./frontmatter";
import { followWikilinks, linkInners, markUnresolved, unresolvedLinks, wikilinks } from "./wikilink";

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
  ".cm-scroller": { fontFamily: "var(--system-font)", fontSize: "15px", lineHeight: "1.6" },
  ".cm-content": { maxWidth: "var(--line-width)", margin: "0 auto", padding: "2rem 1.5rem 40vh" },
  ".cm-panels": {
    backgroundColor: "var(--panel)",
    color: "var(--text)",
    fontFamily: "var(--system-font)",
    fontSize: "13px",
  },
  ".cm-panels-top": { borderBottom: "1px solid var(--separator)" },
  ".cm-search input, .cm-search button": { fontSize: "13px" },
});

/** The system's spelling, autocorrection and text replacements, as in any Mac text field. */
const systemTextServices = EditorView.contentAttributes.of({ spellcheck: "true", autocorrect: "on" });

/** Marks a change that came from disk: not the user's, so not reported back to the app. */
const fromDisk = Annotation.define<boolean>();

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
  // Per transaction: the user's keys may share an update with a reload from disk.
  if (!update.transactions.some((tr) => tr.docChanged && !tr.annotation(fromDisk))) return;
  unreported = true;
  reportTimer ??= setTimeout(() => {
    const message = report();
    if (message) send(message);
  }, reportInterval);
});

/** How long after the last change the editor asks which links are Unresolved. */
const checkDelay = 400;
let checkTimer: ReturnType<typeof setTimeout> | undefined;
/** The links last sent to the app, so an unchanged list is not sent again. */
let checkedInners = "";

function checkLinks(force = false) {
  clearTimeout(checkTimer);
  if (noteID === null) return;
  const inners = linkInners(view.state);
  if (inners === null) {
    // A long Note still parsing: ask again once more of it is parsed.
    checkTimer = setTimeout(() => checkLinks(force), checkDelay);
    return;
  }
  const key = JSON.stringify(inners);
  if (!force && key === checkedInners) return;
  checkedInners = key;
  send({ type: "checkLinks", noteID, inners });
}

const checkLinksAfterChanges = EditorView.updateListener.of((update) => {
  if (!update.docChanged) return;
  clearTimeout(checkTimer);
  checkTimer = setTimeout(() => checkLinks(), checkDelay);
});

function initialState(text: string, editable: boolean) {
  return EditorState.create({
    doc: text,
    extensions: [
      markdown({ base: markdownLanguage, extensions: [frontmatter, wikilinks] }),
      followWikilinks((inner) => send({ type: "followLink", inner })),
      unresolvedLinks,
      checkLinksAfterChanges,
      linkCompletion((requestID) => send({ type: "completeLink", requestID })),
      noteTheme,
      syntaxHighlighting(sourceHighlight),
      EditorView.lineWrapping,
      history(),
      search({ top: true }),
      systemTextServices,
      keymap.of([...searchKeymap, ...defaultKeymap, ...historyKeymap]),
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
        checkLinks(true);
        return null;
      }
      case "clear": {
        const pending = report();
        if (pending) send(pending);
        noteID = null;
        view.setState(initialState("", false));
        return null;
      }
      case "selectAll":
      case "undo":
      case "redo": {
        // In the find bar's fields these act on the field, as in any text field.
        const field = document.activeElement;
        if (field instanceof HTMLInputElement) {
          if (command.type === "selectAll") field.select();
          else document.execCommand(command.type);
          return null;
        }
        ({ selectAll, undo, redo })[command.type](view);
        return null;
      }
      case "unresolvedLinks":
        if (command.noteID === noteID) markUnresolved(view, command.inners);
        return null;
      case "linkCompletions":
        receiveLinkCompletions(command.requestID, command.completions);
        return null;
      case "find":
        if (command.step === "open") openSearchPanel(view);
        else if (command.step === "next") findNext(view);
        else findPrevious(view);
        return null;
      case "reload": {
        if (command.noteID !== noteID) return null;
        if (unreported || command.revision !== revision) {
          // The user typed since the report the app last received: their text stays here, and
          // the app learns of it from this report (or the one already on its way).
          const pending = report();
          if (pending) send(pending);
          return null;
        }
        // Only the part that differs changes, so the cursor and the scroll map through it.
        const doc = view.state.doc.toString();
        const text = command.text;
        let start = 0;
        while (start < doc.length && start < text.length && doc[start] === text[start]) start++;
        let end = 0;
        while (
          end < doc.length - start &&
          end < text.length - start &&
          doc[doc.length - 1 - end] === text[text.length - 1 - end]
        ) end++;
        view.dispatch({
          changes: { from: start, to: doc.length - end, insert: text.slice(start, text.length - end) },
          annotations: [fromDisk.of(true), Transaction.addToHistory.of(false)],
        });
        return "applied";
      }
      case "flush": {
        const message = report();
        return message ? JSON.stringify(message) : null;
      }
    }
  },
};

send({ type: "ready" });
