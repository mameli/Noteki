import { syntaxTree } from "@codemirror/language";
import type { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import TurndownService from "turndown";
import { gfm } from "turndown-plugin-gfm";

// Text copied with its formatting (a web page, Mail, Outlook, Teams, Word, Google Docs) is pasted
// as Markdown, as MarkText does: headings, bold, italic, links, lists, quotes, code and tables.
// Paste and Match Style (⇧⌘V), a paste into code, and text whose HTML carries nothing but its
// look (an editor's colours) stay plain text.

const turndown = new TurndownService({
  headingStyle: "atx",
  bulletListMarker: "-",
  codeBlockStyle: "fenced",
  emDelimiter: "*",
  strongDelimiter: "**",
  hr: "---",
});
turndown.use(gfm);
// Not text: what a page carries besides it.
turndown.remove(["script", "style", "title", "meta", "head"]);
// Google Docs wraps all it copies in a `<b style="font-weight:normal">`: not bold.
turndown.addRule("notBold", {
  filter: (node) => (node.nodeName === "B" || node.nodeName === "STRONG") && /font-weight:\s*(normal|400)/.test(node.getAttribute("style") ?? ""),
  replacement: (content) => content,
});
// Word and Outlook: their empty paragraphs `<o:p>`.
turndown.addRule("office", { filter: (node) => node.nodeName.toLowerCase() === "o:p", replacement: (content) => content });
// A list item: one space after its marker, as Obsidian writes it (Turndown writes three).
turndown.addRule("listItem", {
  filter: "li",
  replacement(content, node, options) {
    const parent = node.parentNode as HTMLElement | null;
    let prefix = `${options.bulletListMarker} `;
    if (parent?.nodeName === "OL") {
      const start = Number(parent.getAttribute("start") ?? "1");
      prefix = `${start + Array.prototype.indexOf.call(parent.children, node)}. `;
    }
    const body = content.replace(/^\n+/, "").replace(/\n+$/, "\n").replace(/\n/gm, `\n${" ".repeat(prefix.length)}`);
    return prefix + body + (node.nextSibling && !/\n$/.test(body) ? "\n" : "");
  },
});

/** Elements that make HTML more than plain text with a look: what is worth turning into Markdown. */
const structure = /<(h[1-6]|b|strong|em|i|a\s|ul|ol|li|blockquote|table|pre|code|del|s|strike|img|hr)\b/i;

/** The Markdown of `html`, or null when it is not worth it: plain text pasted as it is. */
export function markdownOf(html: string, plain: string): string | null {
  if (!structure.test(html)) return null;
  // An editor's code (VS Code, Xcode) is copied as coloured spans and divs, and as its text.
  if (/<pre\b/i.test(html) === false && /^\s*<(meta[^>]*>\s*)?<div[^>]*style="[^"]*white-space:\s*pre/i.test(html)) return null;
  const markdown = turndown.turndown(html).replace(/\n{3,}/g, "\n\n").trim();
  if (markdown === "") return null;
  // Nothing gained over the plain text: keep it, with its line breaks.
  if (markdown.replace(/\s+/g, " ") === plain.replace(/\s+/g, " ").trim()) return null;
  return markdown;
}

/** In code nothing is Markdown: a paste there stays as copied. */
function inCode(state: EditorState, pos: number): boolean {
  for (let node: ReturnType<ReturnType<typeof syntaxTree>["resolveInner"]> | null = syntaxTree(state).resolveInner(pos, -1); node; node = node.parent) {
    if (["FencedCode", "CodeBlock", "InlineCode", "Frontmatter", "Comment", "CommentBlock"].includes(node.name)) return true;
  }
  return false;
}

export const pasteAsMarkdown = EditorView.domEventHandlers({
  paste(event, view) {
    const data = event.clipboardData;
    const html = data?.getData("text/html");
    if (!data || !html || view.state.readOnly) return false;
    if (inCode(view.state, view.state.selection.main.from)) return false;
    const markdown = markdownOf(html, data.getData("text/plain"));
    if (markdown === null) return false;
    event.preventDefault();
    view.dispatch(view.state.replaceSelection(markdown), { userEvent: "input.paste", scrollIntoView: true });
    return true;
  },
});
