import { type EditorState, Facet } from "@codemirror/state";
import { EditorView, WidgetType } from "@codemirror/view";

// The `tags` Property's Tags as chips (SPEC, Frontmatter): a × removes one, a field adds one,
// completed with the Vault's Tags. The app writes the Frontmatter (NotekiCore); the editor only
// says what the user asked for.

/** What the chips ask of the app: given by main.ts, which knows the Note and the bridge. */
export interface TagEditing {
  add(state: EditorState, tag: string): void;
  remove(state: EditorState, tag: string): void;
  /** The Vault's Tags, those most Notes have first. */
  complete(): Promise<string[]>;
}

export const tagEditing = Facet.define<TagEditing, TagEditing | null>({ combine: (values) => values[0] ?? null });

export class TagChipWidget extends WidgetType {
  constructor(readonly name: string) {
    super();
  }

  eq(other: TagChipWidget) {
    return other.name === this.name;
  }

  toDOM(view: EditorView) {
    const chip = document.createElement("span");
    chip.className = "cm-tag-chip";
    chip.textContent = this.name;
    const remove = document.createElement("button");
    remove.className = "cm-tag-remove";
    remove.textContent = "×";
    remove.tabIndex = -1;
    remove.setAttribute("aria-label", `Remove the Tag ${this.name}`);
    // The cursor stays where it was: the YAML does not open.
    remove.addEventListener("mousedown", (event) => event.preventDefault());
    remove.addEventListener("click", (event) => {
      event.preventDefault();
      if (view.state.facet(EditorView.editable)) view.state.facet(tagEditing)?.remove(view.state, this.name);
    });
    chip.append(remove);
    return chip;
  }

  /** A click on the chip's name places the cursor there, in the YAML; the × is the chip's own. */
  ignoreEvent(event: Event) {
    return event.target instanceof Element && event.target.closest(".cm-tag-remove") !== null;
  }
}

/** The Vault's Tags offered while typing one, shared by every field. */
const suggestions = "noteki-tags";

export class AddTagWidget extends WidgetType {
  /** The Tags already there, not offered again. */
  constructor(readonly present: string[]) {
    super();
  }

  eq(other: AddTagWidget) {
    return other.present.join("\n") === this.present.join("\n");
  }

  toDOM(view: EditorView) {
    const box = document.createElement("span");
    box.className = "cm-tag-add";
    const open = document.createElement("button");
    open.textContent = "+";
    open.tabIndex = -1;
    open.setAttribute("aria-label", "Add a Tag");
    const field = document.createElement("input");
    field.type = "text";
    field.placeholder = "Tag";
    field.setAttribute("list", suggestions);
    field.spellcheck = false;
    field.hidden = true;
    const close = () => {
      field.value = "";
      field.hidden = true;
      open.hidden = false;
    };
    open.addEventListener("mousedown", (event) => event.preventDefault());
    open.addEventListener("click", (event) => {
      event.preventDefault();
      const editing = view.state.facet(tagEditing);
      if (!editing || !view.state.facet(EditorView.editable)) return;
      open.hidden = true;
      field.hidden = false;
      field.focus();
      const present = new Set(this.present.map((tag) => tag.toLowerCase()));
      void editing.complete().then((tags) => offer(tags.filter((tag) => !present.has(tag.toLowerCase()))));
    });
    const commit = () => {
      const tag = field.value.trim();
      close();
      view.focus();
      if (tag) view.state.facet(tagEditing)?.add(view.state, tag);
    };
    field.addEventListener("keydown", (event) => {
      if (event.key === "Enter") {
        event.preventDefault();
        commit();
      } else if (event.key === "Escape") {
        event.preventDefault();
        close();
        view.focus();
      }
    });
    // A Tag chosen from the list is added as if typed and confirmed.
    field.addEventListener("input", (event) => {
      if ((event as InputEvent).inputType === "insertReplacementText" || !(event instanceof InputEvent)) commit();
    });
    field.addEventListener("blur", close);
    box.append(open, field);
    return box;
  }

  ignoreEvent() {
    return true;
  }
}

/** The Tags the field's list offers. */
function offer(tags: string[]) {
  let list = document.getElementById(suggestions);
  if (!list) {
    list = document.createElement("datalist");
    list.id = suggestions;
    document.body.append(list);
  }
  list.replaceChildren(...tags.map((tag) => Object.assign(document.createElement("option"), { value: tag })));
}

export const tagChipsTheme = EditorView.baseTheme({
  ".cm-tag-chip": {
    display: "inline-block",
    verticalAlign: "top",
    backgroundColor: "rgba(var(--accent-rgb), 0.15)",
    color: "var(--link)",
    borderRadius: "10px",
    padding: "0 0.2em 0 0.6em",
    marginRight: "0.3em",
  },
  ".cm-tag-remove, .cm-tag-add button": {
    border: "none",
    background: "none",
    color: "inherit",
    font: "inherit",
    padding: "0 0.3em",
    margin: "0",
    lineHeight: "inherit",
    cursor: "default",
    opacity: "0.6",
  },
  ".cm-tag-remove:hover, .cm-tag-add button:hover": { opacity: "1" },
  ".cm-tag-add": { display: "inline-block", verticalAlign: "top", color: "var(--syntax)" },
  ".cm-tag-add input": {
    font: "inherit",
    fontSize: "0.9em",
    width: "12em",
    padding: "0 0.4em",
    margin: "0",
    border: "1px solid var(--separator)",
    borderRadius: "10px",
    background: "var(--background)",
    color: "var(--text)",
    outline: "none",
    boxSizing: "border-box",
    height: "1.4em",
    verticalAlign: "baseline",
  },
});
