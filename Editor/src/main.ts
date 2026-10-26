import { defaultKeymap, history, historyKeymap, redo, selectAll, undo } from "@codemirror/commands";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { findNext, findPrevious, openSearchPanel, search, searchKeymap } from "@codemirror/search";
import { Compartment, EditorState, type StateEffect, Transaction } from "@codemirror/state";
import { EditorView, keymap } from "@codemirror/view";
import { tags } from "@lezer/highlight";
import { type EditorCommand, type EditorMessage, send } from "./bridge";
import { linkCompletion, receiveLinkCompletions } from "./completion";
import { embeddedFile, embedFiles, embedImages, setEmbeds } from "./embeds";
import { bodyStart, frontmatterReading, frontmatterText, setFrontmatter } from "./frontmatter";
import { broughtBack, changeTo, fromDisk } from "./states";
import { type TagEditing, tagEditing } from "./tagchips";
import { highlightTag } from "./highlight";
import { insertion } from "./insert";
import { applySettings } from "./settings";
import { noteLanguage } from "./language";
import { livePreview } from "./livepreview";
import { followWikilinks, linkInners, markUnresolved, unresolvedLinks } from "./wikilink";

// Colours come from the CSS variables in editor.css, so light and dark follow the system.
const sourceHighlight = HighlightStyle.define([
  { tag: tags.heading1, fontSize: "1.6em", fontWeight: "700" },
  { tag: tags.heading2, fontSize: "1.35em", fontWeight: "700" },
  { tag: tags.heading3, fontSize: "1.15em", fontWeight: "700" },
  { tag: [tags.heading4, tags.heading5, tags.heading6], fontWeight: "700" },
  { tag: tags.strong, fontWeight: "700" },
  { tag: tags.emphasis, fontStyle: "italic" },
  { tag: tags.strikethrough, textDecoration: "line-through" },
  { tag: highlightTag, backgroundColor: "var(--highlight)", borderRadius: "2px" },
  // Before links, which win in a quote. Through a variable a callout sets: its text is not grey.
  { tag: tags.quote, color: "var(--quote-text)" },
  { tag: tags.link, color: "var(--link)" },
  { tag: tags.url, color: "var(--link)" },
  { tag: tags.monospace, fontFamily: "var(--mono)" },
  { tag: [tags.processingInstruction, tags.contentSeparator], color: "var(--syntax)" },
  // The Frontmatter: through a variable the Properties table sets, its values in the text's colour.
  { tag: tags.meta, color: "var(--frontmatter-text)" },
  { tag: tags.comment, color: "var(--syntax)" },
  // Code in its language (codelanguages.ts), in Xcode's colours.
  { tag: [tags.keyword, tags.modifier, tags.controlKeyword, tags.operatorKeyword], color: "var(--code-keyword)" },
  { tag: [tags.string, tags.special(tags.string), tags.regexp, tags.character], color: "var(--code-string)" },
  { tag: [tags.number, tags.bool, tags.null, tags.atom], color: "var(--code-number)" },
  { tag: [tags.typeName, tags.className, tags.namespace, tags.standard(tags.variableName)], color: "var(--code-type)" },
  { tag: [tags.function(tags.variableName), tags.function(tags.propertyName), tags.definition(tags.variableName)], color: "var(--code-function)" },
  { tag: [tags.propertyName, tags.attributeName, tags.tagName], color: "var(--code-property)" },
]);

// Set here rather than in editor.css: CodeMirror's base theme would win over a stylesheet.
const noteTheme = EditorView.theme({
  ".cm-scroller": { fontFamily: "var(--text-font)", fontSize: "var(--text-size)", lineHeight: "1.6" },
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

/** How often, at most, the text is reported while the user types. The app saves after it. */
const reportInterval = 250;

/** The Note shown, by its path; null before the first one. */
let noteID: string | null = null;
let unreported = false;
let revision = 0;
/** The last revision reported for each Note: a reload is for the text of that report. */
const reported = new Map<string, number>();
let reportTimer: ReturnType<typeof setTimeout> | undefined;

function report(): EditorMessage | null {
  clearTimeout(reportTimer);
  reportTimer = undefined;
  if (noteID === null || !unreported) return null;
  unreported = false;
  revision += 1;
  reported.set(noteID, revision);
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

/** The Frontmatter last asked about, so an unchanged one is not asked about again. */
let askedFrontmatter: string | null = null;

/** Asks the app how the Frontmatter reads: it draws the Properties table from the answer. */
function readFrontmatter() {
  if (noteID === null) return;
  const text = frontmatterText(view.state);
  if (text === null || text === askedFrontmatter) return;
  askedFrontmatter = text;
  send({ type: "readFrontmatter", noteID, text });
}

const readFrontmatterAfterChanges = EditorView.updateListener.of((update) => {
  if (update.docChanged) readFrontmatter();
});

/** The Tag completions asked for and not answered yet, by request. */
const tagRequests = new Map<number, (tags: string[]) => void>();
let lastTagRequest = 0;

/** A Tag added or removed asks the app to write the Frontmatter; its answer replaces it. */
const tagEditingByApp: TagEditing = {
  add(state, tag) {
    const text = frontmatterText(state);
    if (noteID !== null && text !== null) send({ type: "addTag", noteID, text, tag });
  },
  remove(state, tag) {
    const text = frontmatterText(state);
    if (noteID !== null && text !== null) send({ type: "removeTag", noteID, text, tag });
  },
  complete() {
    lastTagRequest += 1;
    const requestID = lastTagRequest;
    send({ type: "completeTag", requestID });
    return new Promise((resolve) => tagRequests.set(requestID, resolve));
  },
};

/** Live preview, or nothing in Source mode, which the app chooses. */
const preview = new Compartment();
let sourceMode = false;
const previewed = () => (sourceMode ? [] : [livePreview, embedImages]);

function initialState(text: string, editable: boolean) {
  return EditorState.create({
    doc: text,
    extensions: [
      noteLanguage,
      // What the app said about the Embeds stays across Source mode: the images come back with it.
      embedFiles,
      // The app's answer too: the table comes back with Live preview.
      frontmatterReading,
      preview.of(previewed()),
      // An image drawn under its line hides its source: a click there places the cursor.
      followWikilinks(
        (inner, newTab) => send({ type: "followLink", inner, newTab }),
        (state, node) => !(node.name === "Embed" && embeddedFile(state, node)?.kind === "image"),
      ),
      unresolvedLinks,
      checkLinksAfterChanges,
      readFrontmatterAfterChanges,
      tagEditing.of(tagEditingByApp),
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

/** The cursor at the start of `number`, from 1, in the middle of the view. */
function goToLine(number: number) {
  const line = view.state.doc.line(Math.min(Math.max(number, 1), view.state.doc.lines));
  view.dispatch({ selection: { anchor: line.from }, effects: EditorView.scrollIntoView(line.from, { y: "center" }) });
}

if (window.notekiSettings) applySettings(window.notekiSettings);

const view = new EditorView({
  state: initialState("", false),
  parent: document.getElementById("editor")!,
});

/**
 * The Notes of the Tabs behind, by their noteID: their state and where they were scrolled, kept
 * as the line at the top of the editor: in pixels it would come back off by the lines' heights
 * CodeMirror only estimates until it draws them.
 */
const kept = new Map<string, { state: EditorState; scroll: StateEffect<unknown> }>();

/** Reports what is not reported yet and keeps the Note shown for when its Tab comes back. */
function keepShown() {
  const pending = report();
  if (pending) send(pending);
  if (noteID !== null) kept.set(noteID, { state: view.state, scroll: view.scrollSnapshot() });
}

/** What every Note shown asks the app, once in the editor. */
function askAboutShown() {
  checkLinks(true);
  askedFrontmatter = null;
  readFrontmatter();
}

window.noteki = {
  receive(command: EditorCommand) {
    switch (command.type) {
      case "load": {
        // The app flushes before switching Note: this only catches what slipped through.
        keepShown();
        noteID = command.noteID;
        view.setState(initialState(command.text, true));
        view.scrollDOM.scrollTop = 0;
        // A Backlink or a search result: the cursor on its line. Otherwise where the text starts,
        // after the Frontmatter, which stays a table.
        if (command.line !== undefined) goToLine(command.line);
        else view.dispatch({ selection: { anchor: bodyStart(view.state) } });
        // Ready for typing when the web view has the keyboard: the app gives it after the Quick
        // switcher or a Backlink, while Files keeps it after a click there.
        view.focus();
        askAboutShown();
        return null;
      }
      case "restore": {
        // A Tab back in front: its Note as it was left, with what the file changed since. Still
        // in the editor when only an Attachment was in front meanwhile.
        if (command.noteID === noteID) {
          const pending = report();
          if (pending) send(pending);
          const change = changeTo(view.state.doc.toString(), command.text);
          view.dispatch({
            ...(change ? { changes: change, annotations: [fromDisk.of(true), Transaction.addToHistory.of(false)] } : {}),
            effects: preview.reconfigure(previewed()),
          });
          view.focus();
          askAboutShown();
          return null;
        }
        const back = kept.get(command.noteID);
        keepShown();
        kept.delete(command.noteID);
        noteID = command.noteID;
        if (!back) {
          view.setState(initialState(command.text, true));
          view.dispatch({ selection: { anchor: bodyStart(view.state) } });
        } else {
          view.setState(broughtBack(back.state, command.text));
          // Live preview or Source mode as the window has it now.
          view.dispatch({ effects: [preview.reconfigure(previewed()), back.scroll] });
        }
        view.focus();
        askAboutShown();
        return null;
      }
      case "drop":
        for (const id of command.noteIDs) {
          kept.delete(id);
          reported.delete(id);
        }
        return null;
      case "clear": {
        const pending = report();
        if (pending) send(pending);
        noteID = null;
        kept.clear();
        reported.clear();
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
      case "frontmatter":
        if (command.noteID === noteID) view.dispatch({ effects: setFrontmatter.of({ text: command.text, reading: command.reading ?? null }) });
        return null;
      case "replaceFrontmatter": {
        // Only over the Frontmatter it was written from, at the top: one typed since is not
        // overwritten.
        if (command.noteID !== noteID || frontmatterText(view.state) !== command.text) return null;
        view.dispatch({ changes: { from: 0, to: command.text.length, insert: command.replacement }, userEvent: "input" });
        return null;
      }
      case "tagCompletions":
        tagRequests.get(command.requestID)?.(command.tags);
        tagRequests.delete(command.requestID);
        return null;
      case "embeds":
        if (command.noteID === noteID) view.dispatch({ effects: setEmbeds.of(command.files) });
        return null;
      case "sourceMode":
        sourceMode = command.on;
        view.dispatch({ effects: preview.reconfigure(previewed()) });
        return null;
      case "goToLine":
        if (command.noteID === noteID) {
          goToLine(command.line);
          view.focus();
        }
        return null;
      case "settings":
        applySettings(command.settings);
        // Lines change height with the font: CodeMirror measures them again, and once more when
        // the new font has loaded, if it was not yet.
        view.requestMeasure();
        void document.fonts.ready.then(() => view.requestMeasure());
        return null;
      case "insert":
        if (command.noteID === noteID) {
          const at = command.point ? view.posAtCoords(command.point, false) : undefined;
          view.dispatch(insertion(view.state, command.text, at));
          view.focus();
        }
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
        if (unreported || command.revision !== (reported.get(command.noteID) ?? 0)) {
          // The user typed since the report the app last received: their text stays here, and
          // the app learns of it from this report (or the one already on its way).
          const pending = report();
          if (pending) send(pending);
          return null;
        }
        // Only the part that differs changes, so the cursor and the scroll map through it.
        const change = changeTo(view.state.doc.toString(), command.text);
        if (change) view.dispatch({ changes: change, annotations: [fromDisk.of(true), Transaction.addToHistory.of(false)] });
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
