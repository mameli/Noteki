import {
  closeSearchPanel,
  findNext,
  findPrevious,
  getSearchQuery,
  replaceAll,
  replaceNext,
  SearchQuery,
  setSearchQuery,
} from "@codemirror/search";
import type { EditorState } from "@codemirror/state";
import { EditorView, type Panel, type ViewUpdate } from "@codemirror/view";

// The find bar (⌘F), drawn as a Mac app's: one row with the field, the count of matches, the
// options as small toggles and the arrows; the replace row only when asked for. It replaces
// CodeMirror's own panel and drives the same search state, so ⌘G and ⇧⌘G go on working.

/** Matches counted at most: past it the count says "1000+". */
const countLimit = 1000;

/** Which match the main selection is, from 1 (0 when it is none), and how many there are. */
export function matchCount(state: EditorState, query: SearchQuery): { current: number; total: number } {
  if (!query.valid || query.search === "") return { current: 0, total: 0 };
  const { from, to } = state.selection.main;
  let total = 0;
  let current = 0;
  const cursor = query.getCursor(state);
  for (let next = cursor.next(); !next.done; next = cursor.next()) {
    total++;
    if (next.value.from === from && next.value.to === to) current = total;
    if (total >= countLimit) break;
  }
  return { current, total };
}

function element<K extends keyof HTMLElementTagNameMap>(tag: K, className: string, attributes: Record<string, string> = {}) {
  const node = document.createElement(tag);
  node.className = className;
  for (const [name, value] of Object.entries(attributes)) node.setAttribute(name, value);
  return node;
}

export function createFindBar(view: EditorView): Panel {
  const dom = element("div", "cm-findbar");
  const findRow = element("div", "cm-findbar-row");
  const replaceRow = element("div", "cm-findbar-row cm-findbar-replace");
  dom.append(findRow, replaceRow);

  const disclosure = element("button", "cm-findbar-icon cm-findbar-disclosure", { type: "button", title: "Show Replace", "aria-label": "Show Replace", "aria-expanded": "false" });
  disclosure.textContent = "›";
  const field = element("label", "cm-findbar-field");
  const glass = element("span", "cm-findbar-glass", { "aria-hidden": "true" });
  glass.textContent = "⌕";
  const input = element("input", "cm-findbar-input", { type: "search", placeholder: "Find", "main-field": "true", "aria-label": "Find", spellcheck: "false" });
  const count = element("span", "cm-findbar-count", { "aria-live": "polite" });
  const toggle = (label: string, title: string) => {
    const button = element("button", "cm-findbar-toggle", { type: "button", title, "aria-label": title, "aria-pressed": "false" });
    button.textContent = label;
    return button;
  };
  const caseToggle = toggle("Aa", "Match Case");
  const regexpToggle = toggle(".*", "Regular Expression");
  const wordToggle = toggle("ab", "Whole Word");
  field.append(glass, input, count, caseToggle, regexpToggle, wordToggle);
  const arrow = (label: string, title: string, run: () => void) => {
    const button = element("button", "cm-findbar-icon", { type: "button", title, "aria-label": title });
    button.textContent = label;
    button.addEventListener("click", run);
    return button;
  };
  const previous = arrow("‹", "Previous (⇧↩)", () => findPrevious(view));
  const next = arrow("›", "Next (↩)", () => findNext(view));
  const arrows = element("div", "cm-findbar-arrows");
  arrows.append(previous, next);
  const done = element("button", "cm-findbar-done", { type: "button" });
  done.textContent = "Done";
  done.addEventListener("click", () => closeSearchPanel(view));
  findRow.append(disclosure, field, arrows, done);

  const replaceInput = element("input", "cm-findbar-input cm-findbar-replace-input", { type: "text", placeholder: "Replace", "aria-label": "Replace", spellcheck: "false" });
  const replaceButton = element("button", "cm-findbar-button", { type: "button" });
  replaceButton.textContent = "Replace";
  replaceButton.addEventListener("click", () => replaceNext(view));
  const replaceAllButton = element("button", "cm-findbar-button", { type: "button" });
  replaceAllButton.textContent = "All";
  replaceAllButton.addEventListener("click", () => replaceAll(view));
  const spacer = element("span", "cm-findbar-spacer");
  replaceRow.append(spacer, replaceInput, replaceButton, replaceAllButton);

  let replacing = false;
  const showReplace = (on: boolean) => {
    replacing = on;
    dom.classList.toggle("cm-findbar-replacing", on);
    disclosure.setAttribute("aria-expanded", String(on));
    disclosure.title = on ? "Hide Replace" : "Show Replace";
  };
  disclosure.addEventListener("click", () => {
    showReplace(!replacing);
    (replacing ? replaceInput : input).focus();
  });

  /** The fields' query to the editor, when it is another. */
  const commit = () => {
    const query = new SearchQuery({
      search: input.value,
      caseSensitive: caseToggle.getAttribute("aria-pressed") === "true",
      regexp: regexpToggle.getAttribute("aria-pressed") === "true",
      wholeWord: wordToggle.getAttribute("aria-pressed") === "true",
      replace: replaceInput.value,
    });
    if (!query.eq(getSearchQuery(view.state))) view.dispatch({ effects: setSearchQuery.of(query) });
  };
  for (const button of [caseToggle, regexpToggle, wordToggle]) {
    // The field keeps the keyboard.
    button.addEventListener("mousedown", (event) => event.preventDefault());
    button.addEventListener("click", () => {
      button.setAttribute("aria-pressed", String(button.getAttribute("aria-pressed") !== "true"));
      commit();
    });
  }
  input.addEventListener("input", commit);
  replaceInput.addEventListener("input", commit);
  input.addEventListener("keydown", (event) => {
    if (event.key !== "Enter" || event.isComposing) return;
    event.preventDefault();
    if (event.shiftKey) findPrevious(view);
    else findNext(view);
  });
  replaceInput.addEventListener("keydown", (event) => {
    if (event.key !== "Enter" || event.isComposing) return;
    event.preventDefault();
    if (event.metaKey) replaceAll(view);
    else replaceNext(view);
  });
  dom.addEventListener("keydown", (event) => {
    if (event.key !== "Escape") return;
    event.preventDefault();
    closeSearchPanel(view);
    view.focus();
  });

  /** The fields and the count, from the editor's query: ⌘F with a selection sets a new one. */
  const show = (state: EditorState) => {
    const query = getSearchQuery(state);
    if (input.value !== query.search) input.value = query.search;
    if (replaceInput.value !== query.replace) replaceInput.value = query.replace;
    caseToggle.setAttribute("aria-pressed", String(query.caseSensitive));
    regexpToggle.setAttribute("aria-pressed", String(query.regexp));
    wordToggle.setAttribute("aria-pressed", String(query.wholeWord));
    field.classList.toggle("cm-findbar-invalid", query.search !== "" && !query.valid);
    const { current, total } = matchCount(state, query);
    count.textContent = query.search === "" ? "" : total === 0 ? "No matches" : `${current || "–"} of ${total >= countLimit ? `${countLimit}+` : total}`;
    previous.disabled = next.disabled = total === 0;
    replaceButton.disabled = replaceAllButton.disabled = total === 0;
  };
  show(view.state);

  return {
    dom,
    top: true,
    mount() {
      input.select();
    },
    update(update: ViewUpdate) {
      const queryChanged = update.transactions.some((tr) => tr.effects.some((effect) => effect.is(setSearchQuery)));
      if (queryChanged || update.docChanged || update.selectionSet) show(update.state);
    },
  };
}

export const findBarTheme = EditorView.baseTheme({
  ".cm-findbar": {
    fontFamily: "var(--system-font)",
    fontSize: "13px",
    padding: "6px 10px",
    backgroundColor: "var(--panel)",
    color: "var(--text)",
  },
  ".cm-findbar-row": { display: "flex", alignItems: "center", gap: "6px" },
  ".cm-findbar-replace": { display: "none", marginTop: "6px" },
  ".cm-findbar-replacing .cm-findbar-replace": { display: "flex" },
  ".cm-findbar-field": {
    flex: "1",
    display: "flex",
    alignItems: "center",
    gap: "2px",
    minWidth: "0",
    height: "24px",
    padding: "0 4px 0 6px",
    borderRadius: "6px",
    border: "1px solid var(--separator)",
    backgroundColor: "var(--background)",
  },
  ".cm-findbar-field:focus-within": { outline: "3px solid color-mix(in srgb, rgb(var(--accent-rgb)) 45%, transparent)", borderColor: "rgb(var(--accent-rgb))" },
  ".cm-findbar-invalid": { borderColor: "var(--error)" },
  ".cm-findbar-glass": { color: "var(--syntax)", fontSize: "15px", lineHeight: "1" },
  ".cm-findbar-input": {
    flex: "1",
    minWidth: "0",
    border: "none",
    outline: "none",
    background: "transparent",
    color: "inherit",
    font: "inherit",
    padding: "0 2px",
  },
  ".cm-findbar-input::-webkit-search-cancel-button": { display: "none" },
  ".cm-findbar-replace-input": {
    flex: "1",
    height: "24px",
    padding: "0 8px",
    borderRadius: "6px",
    border: "1px solid var(--separator)",
    backgroundColor: "var(--background)",
  },
  ".cm-findbar-replace-input:focus": { outline: "3px solid color-mix(in srgb, rgb(var(--accent-rgb)) 45%, transparent)", borderColor: "rgb(var(--accent-rgb))" },
  ".cm-findbar-count": { color: "var(--syntax)", fontSize: "11px", whiteSpace: "nowrap", padding: "0 4px", fontVariantNumeric: "tabular-nums" },
  ".cm-findbar button": {
    font: "inherit",
    color: "inherit",
    border: "none",
    background: "transparent",
    borderRadius: "4px",
    cursor: "default",
  },
  ".cm-findbar button:disabled": { opacity: "0.35" },
  ".cm-findbar-toggle": { fontSize: "11px !important", fontWeight: "600", padding: "1px 4px", color: "var(--syntax) !important" },
  ".cm-findbar-toggle:hover, .cm-findbar-icon:hover:not(:disabled)": { backgroundColor: "color-mix(in srgb, var(--text) 10%, transparent) !important" },
  ".cm-findbar-toggle[aria-pressed=true]": { backgroundColor: "rgb(var(--accent-rgb)) !important", color: "white !important" },
  ".cm-findbar-arrows": { display: "flex", border: "1px solid var(--separator)", borderRadius: "6px", overflow: "hidden" },
  ".cm-findbar-arrows .cm-findbar-icon": { borderRadius: "0" },
  ".cm-findbar-arrows .cm-findbar-icon + .cm-findbar-icon": { borderLeft: "1px solid var(--separator) !important" },
  ".cm-findbar-icon": { width: "26px", height: "22px", fontSize: "17px !important", lineHeight: "1" },
  ".cm-findbar-disclosure": { width: "18px", transition: "transform 120ms ease", color: "var(--syntax) !important" },
  ".cm-findbar-replacing .cm-findbar-disclosure": { transform: "rotate(90deg)" },
  ".cm-findbar-spacer": { width: "18px", flex: "none" },
  ".cm-findbar-done, .cm-findbar-button": {
    height: "24px",
    padding: "0 10px",
    border: "1px solid var(--separator) !important",
    backgroundColor: "var(--background) !important",
  },
  ".cm-findbar-done:hover, .cm-findbar-button:hover:not(:disabled)": { backgroundColor: "color-mix(in srgb, var(--text) 8%, var(--background)) !important" },
});
