import {
  autocompletion,
  type Completion,
  type CompletionContext,
  type CompletionResult,
  pickedCompletion,
} from "@codemirror/autocomplete";
import { syntaxTree } from "@codemirror/language";
import type { SyntaxNode } from "@lezer/common";
import { EditorView } from "@codemirror/view";
import type { LinkCompletion, LinkCompletions } from "./bridge";

// Typing `[[` offers the Vault's Notes. The app knows them and how each must be written; the
// editor asks once per `[[` and then filters as the user types, fuzzily on the name, as
// CodeMirror's completion does.

/**
 * What a link's name can hold: not a bracket, nor the start of an Alias, heading or block, nor
 * a backtick, where the parser stops seeing a link.
 */
const nameChars = "[^\\[\\]|#^`\\n]*";
/** `[[` and the name typed so far, before the cursor; not `[[[`, which is no link. */
const typedLink = new RegExp(`(?<!\\[)\\[\\[(${nameChars})$`);
const typedName = new RegExp(`^${nameChars}$`);
/** After the cursor, in a link being edited: the rest of its name, an Alias or heading, `]]`. */
const restOfLink = new RegExp(`^(${nameChars})([|#^][^\\[\\]\`\\n]*)?\\]\\]`);

/** How long the editor waits for the app's answer before showing nothing. */
const answerTimeout = 2000;
let lastRequest = 0;
const waiting = new Map<number, { resolve: (completions: LinkCompletions | null) => void; timer: number }>();

/** The app's answer to a `completeLink`; a late one, after the timeout, is dropped. */
export function receiveLinkCompletions(requestID: number, completions: LinkCompletions) {
  const request = waiting.get(requestID);
  if (!request) return;
  waiting.delete(requestID);
  clearTimeout(request.timer);
  request.resolve(completions);
}

function ask(send: (requestID: number) => void): Promise<LinkCompletions | null> {
  const requestID = ++lastRequest;
  return new Promise((resolve) => {
    const timer = setTimeout(() => {
      waiting.delete(requestID);
      resolve(null);
    }, answerTimeout);
    waiting.set(requestID, { resolve, timer });
    send(requestID);
  });
}

const codeNodes = new Set(["InlineCode", "FencedCode", "CodeBlock"]);

function inCode(context: CompletionContext): boolean {
  for (let node: SyntaxNode | null = syntaxTree(context.state).resolveInner(context.pos, -1); node; node = node.parent) {
    if (codeNodes.has(node.name)) return true;
  }
  return false;
}

/**
 * Writes the chosen Note's link and closes the brackets, or, in a link already closed, replaces
 * its name and keeps its Alias or heading. The cursor goes after the link, or before the Alias.
 */
function writeLink(link: string) {
  return (view: EditorView, completion: Completion, from: number, to: number) => {
    const line = view.state.doc.lineAt(to);
    const rest = restOfLink.exec(view.state.sliceDoc(to, line.to));
    const end = rest ? to + rest[1].length : to;
    const insert = rest ? link : `${link}]]`;
    const cursor = from + link.length + (rest?.[2] ? 0 : 2);
    view.dispatch({
      changes: { from, to: end, insert },
      selection: { anchor: cursor },
      annotations: pickedCompletion.of(completion),
      userEvent: "input.complete",
      scrollIntoView: true,
    });
  };
}

function option(completion: LinkCompletion, boost: number): Completion {
  return { label: completion.name, detail: completion.folder, boost, apply: writeLink(completion.link) };
}

function linkSource(send: (requestID: number) => void) {
  return async (context: CompletionContext): Promise<CompletionResult | null> => {
    const line = context.state.doc.lineAt(context.pos);
    const typed = typedLink.exec(line.text.slice(0, context.pos - line.from));
    if (!typed || inCode(context)) return null;
    const completions = await ask(send);
    if (!completions || context.aborted) return null;
    // Recent Notes first, the latest first, among those the typing matches as well: CodeMirror
    // ranks by how well the name matches, in steps of 100, and the boost stays below that.
    const options = [
      ...completions.recent.map((completion, rank) => option(completion, Math.max(1, 99 - rank))),
      ...completions.others.map((completion) => option(completion, 0)),
    ];
    if (options.length === 0) return null;
    return { from: context.pos - typed[1].length, options, validFor: typedName };
  };
}

export function linkCompletion(send: (requestID: number) => void) {
  return [
    autocompletion({ override: [linkSource(send)], icons: false }),
    EditorView.theme({
      ".cm-tooltip.cm-tooltip-autocomplete": {
        backgroundColor: "var(--panel)",
        border: "1px solid var(--separator)",
        borderRadius: "6px",
        overflow: "hidden",
        fontFamily: "var(--system-font)",
        fontSize: "13px",
      },
      ".cm-tooltip.cm-tooltip-autocomplete > ul": { fontFamily: "inherit", maxHeight: "16em" },
      ".cm-tooltip.cm-tooltip-autocomplete > ul > li": { padding: "2px 8px" },
      ".cm-tooltip.cm-tooltip-autocomplete > ul > li[aria-selected]": { backgroundColor: "var(--link)", color: "var(--on-link)" },
      ".cm-completionDetail": { marginLeft: "0.8em", fontStyle: "normal", opacity: "0.6" },
      ".cm-completionMatchedText": { textDecoration: "none", fontWeight: "600" },
    }),
  ];
}
