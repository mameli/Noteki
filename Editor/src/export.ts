import { syntaxTree } from "@codemirror/language";
import type { EditorState } from "@codemirror/state";
import MarkdownIt from "markdown-it";
import footnote from "markdown-it-footnote";
import { calloutKind } from "./callouts";
import { embedFiles, type EmbeddedFile } from "./embeds";
import { formulas, type Katex } from "./math";

// Export as HTML or PDF, and Print (SPEC, Export): the Note as one page of HTML, standing alone,
// drawn as Live preview draws it: Headings, emphasis, highlights, links, Wikilinks as their name
// or Alias, images, tasks, tables, callouts, code, footnotes, formulas and Mermaid diagrams. The
// Frontmatter and Comments are left out: they are not read. Images keep the address the editor
// loads them from (`noteki-file://vault/…`); the app puts each file's data in its place.

const placeholder = (kind: string, index: number) => `NOTEKI${kind}${index}X`;

/** What the export needs from the page: KaTeX and Mermaid, loaded only for a Note that has them. */
export interface Renderers {
  katex?: () => Promise<Katex>;
  mermaid?: (source: string) => Promise<string>;
  parse?: (html: string) => Document;
}

/** The Note's text as markdown-it reads it: without its Frontmatter and Comments, formulas as placeholders. */
function prepared(state: EditorState): { text: string; formulas: { tex: string; display: boolean }[] } {
  const cuts: { from: number; to: number; insert: string }[] = [];
  syntaxTree(state).iterate({
    enter(node) {
      if (node.name === "Frontmatter" || node.name === "Comment" || node.name === "CommentBlock") {
        cuts.push({ from: node.from, to: node.to, insert: "" });
        return false;
      }
      return undefined;
    },
  });
  const found = formulas(state);
  found.forEach((formula, index) => {
    const mark = placeholder("MATH", index);
    cuts.push({ from: formula.from, to: formula.to, insert: formula.display ? `\n\n${mark}\n\n` : mark });
  });
  cuts.sort((a, b) => b.from - a.from);
  let text = state.doc.toString();
  let last = Infinity;
  for (const cut of cuts) {
    // A formula inside a Comment goes with the Comment.
    if (cut.to > last) continue;
    text = text.slice(0, cut.from) + cut.insert + text.slice(cut.to);
    last = cut.from;
  }
  return { text, formulas: found.map(({ tex, display }) => ({ tex, display })) };
}

function escape(text: string): string {
  return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

/** Where the editor loads a file of the Vault from, as embeds.ts draws it. */
function fileAddress(file: EmbeddedFile): string {
  return file.url ?? "noteki-file://vault/" + file.path.split("/").map(encodeURIComponent).join("/");
}

/** The Note as the body of a page of HTML. */
export async function noteBody(state: EditorState, renderers: Renderers = {}): Promise<string> {
  const { text, formulas: found } = prepared(state);
  const files = state.field(embedFiles, false) ?? {};
  const diagrams: string[] = [];
  // A line break is one, as in Obsidian (its "strict line breaks" off) and in the editor.
  const md = new MarkdownIt({ html: true, linkify: true, typographer: false, breaks: true }).use(footnote);
  // A reference shows its number alone, also the second time: `1`, not `[1:1]`.
  md.renderer.rules.footnote_caption = (tokens, index) => String(Number(tokens[index].meta.id) + 1);
  const fence = md.renderer.rules.fence!;
  md.renderer.rules.fence = (tokens, index, options, env, self) => {
    const token = tokens[index];
    if (token.info.trim().toLowerCase() === "mermaid" && renderers.mermaid) {
      diagrams.push(token.content);
      return `<div class="mermaid">${placeholder("DIAGRAM", diagrams.length - 1)}</div>\n`;
    }
    return fence(tokens, index, options, env, self);
  };
  let html = md.render(text);

  const parse = renderers.parse ?? ((source: string) => new DOMParser().parseFromString(source, "text/html"));
  const doc = parse(`<!doctype html><html><body>${html}</body></html>`);
  const body = doc.body;

  // In the text, not in code: Embeds, Wikilinks, highlights.
  const walker = doc.createTreeWalker(body, 4 /* NodeFilter.SHOW_TEXT */);
  const texts: Text[] = [];
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    if (!(node.parentElement?.closest("code, pre"))) texts.push(node as Text);
  }
  const inline = /(!?)\[\[([^\]\n]+?)\]\]|==([^=\n]+?)==/g;
  for (const node of texts) {
    const value = node.nodeValue ?? "";
    if (!inline.test(value)) continue;
    inline.lastIndex = 0;
    const replaced = value.replace(inline, (all, bang: string, inner: string | undefined, marked: string | undefined) => {
      if (marked !== undefined) return `<mark>${escape(marked)}</mark>`;
      const [target, alias] = [inner!.split("|")[0], inner!.split("|").slice(1).join("|")];
      if (bang) {
        const file = files[inner!];
        if (file?.kind === "image") {
          const width = /^\d+$/.test(alias) ? ` width="${alias}"` : "";
          return `<img src="${escape(fileAddress(file))}" alt="${escape(target)}"${width}>`;
        }
        return `<span class="wikilink">${escape(alias || target)}</span>`;
      }
      return `<span class="wikilink">${escape(alias || target.replace(/#\^?/, " › "))}</span>`;
    });
    const holder = doc.createElement("span");
    holder.innerHTML = replaced;
    node.replaceWith(...Array.from(holder.childNodes));
  }

  // Markdown images of the Vault: from where the editor loads them.
  for (const image of Array.from(body.querySelectorAll("img"))) {
    const src = image.getAttribute("src") ?? "";
    if (/^(https?:|data:|noteki-file:)/i.test(src)) continue;
    let inner = src;
    try {
      inner = decodeURIComponent(src);
    } catch {}
    const file = files[inner];
    if (file) image.setAttribute("src", fileAddress(file));
  }

  // Tasks: a checkbox, ticked as the file says.
  for (const item of Array.from(body.querySelectorAll("li"))) {
    const first = item.firstChild?.nodeName === "P" ? item.firstChild.firstChild : item.firstChild;
    if (!first || first.nodeType !== 3) continue;
    const found = /^\[([ xX])\]\s/.exec(first.nodeValue ?? "");
    if (!found) continue;
    first.nodeValue = (first.nodeValue ?? "").slice(found[0].length);
    const box = doc.createElement("input");
    box.setAttribute("type", "checkbox");
    box.setAttribute("disabled", "");
    item.classList.add("task");
    if (found[1] !== " ") {
      box.setAttribute("checked", "");
      item.classList.add("done");
    }
    first.parentNode?.insertBefore(box, first);
  }

  // Callouts: a quote whose first line is `[!type] Title`.
  for (const quote of Array.from(body.querySelectorAll("blockquote"))) {
    if (quote.parentElement?.closest("blockquote")) continue;
    const first = Array.from(quote.children).find((child) => child.nodeName === "P");
    const lead = first?.firstChild;
    if (!first || !lead || lead.nodeType !== 3) continue;
    const found = /^\[!([\w-]+)\][+-]?[ \t]*([^\n]*)\n?/.exec(lead.nodeValue ?? "");
    if (!found) continue;
    const kind = calloutKind(found[1]);
    lead.nodeValue = (lead.nodeValue ?? "").slice(found[0].length);
    const title = doc.createElement("div");
    title.className = "callout-title";
    title.textContent = found[2].trim() || found[1][0].toUpperCase() + found[1].slice(1);
    const box = doc.createElement("div");
    box.className = `callout callout-${kind}`;
    box.appendChild(title);
    for (const child of Array.from(quote.childNodes)) box.appendChild(child);
    if (first.textContent?.trim() === "") first.remove();
    quote.replaceWith(box);
  }

  html = body.innerHTML;
  if (found.length) {
    const katex = renderers.katex ? await renderers.katex().catch(() => null) : null;
    found.forEach((formula, index) => {
      const drawn = katex
        ? katex.renderToString(formula.tex, { displayMode: formula.display, throwOnError: false, output: "html" })
        : `<code>${escape(formula.tex)}</code>`;
      const mark = placeholder("MATH", index);
      html = formula.display ? html.replace(`<p>${mark}</p>`, `<div class="math">${drawn}</div>`).replace(mark, drawn) : html.replace(mark, drawn);
    });
  }
  for (const [index, source] of diagrams.entries()) {
    let drawn: string;
    try {
      drawn = await renderers.mermaid!(source);
    } catch {
      drawn = `<pre><code>${escape(source)}</code></pre>`;
    }
    html = html.replace(placeholder("DIAGRAM", index), drawn);
  }
  return html;
}

/** The colours of a page to read and print: light, whatever the editor's theme. */
const style = `
:root { color-scheme: light; }
body { font: 15px/1.6 -apple-system, BlinkMacSystemFont, "Helvetica Neue", sans-serif; color: #24292f; background: #fff;
  max-width: 46em; margin: 2.5em auto; padding: 0 1.5em; overflow-wrap: break-word; }
h1, h2, h3, h4, h5, h6 { line-height: 1.25; margin: 1.4em 0 0.5em; }
h1 { font-size: 1.8em; } h2 { font-size: 1.45em; } h3 { font-size: 1.2em; }
a, .wikilink { color: #0969da; text-decoration: none; }
code { font: 0.9em ui-monospace, "SF Mono", Menlo, monospace; background: #f3f4f6; border-radius: 4px; padding: 0.1em 0.3em; }
pre { background: #f6f8fa; border-radius: 6px; padding: 0.8em 1em; overflow-x: auto; white-space: pre-wrap; }
pre code { background: none; padding: 0; }
blockquote { margin: 1em 0; padding: 0 1em; color: #57606a; border-left: 3px solid #d0d7de; }
table { border-collapse: collapse; margin: 1em 0; }
th, td { border: 1px solid #d0d7de; padding: 0.35em 0.7em; text-align: left; vertical-align: top; }
th { background: #f6f8fa; }
img, svg { max-width: 100%; height: auto; }
mark { background: #fff2a8; padding: 0 0.1em; }
hr { border: none; border-top: 1px solid #d0d7de; margin: 1.5em 0; }
li.task { list-style: none; margin-left: -1.3em; }
li.task input { margin: 0 0.45em 0 0; }
li.done { color: #8c959f; text-decoration: line-through; }
.math { overflow-x: auto; margin: 1em 0; }
.mermaid { margin: 1em 0; text-align: center; }
.callout { margin: 1em 0; padding: 0.6em 1em; border-radius: 6px; border-left: 4px solid var(--c); background: color-mix(in srgb, var(--c) 9%, white); }
.callout-title { font-weight: 600; color: var(--c); margin-bottom: 0.2em; }
.callout > p:last-child { margin-bottom: 0.2em; }
.callout-note, .callout-info, .callout-todo { --c: #0969da; }
.callout-abstract, .callout-tip { --c: #1a7f72; }
.callout-success { --c: #1a7f37; }
.callout-question, .callout-warning { --c: #bc4c00; }
.callout-failure, .callout-danger, .callout-bug { --c: #cf222e; }
.callout-example { --c: #8250df; }
.callout-quote { --c: #6e7781; }
.footnotes { font-size: 0.9em; color: #57606a; }
.footnote-ref a { text-decoration: none; }
@media print { body { margin: 0; max-width: none; } pre, table, img, svg, .callout { break-inside: avoid; } }
`;

/** The page of HTML: the Note's body under its name, KaTeX's stylesheet when it has formulas. */
export function page(title: string, body: string, katexStyle: string | null): string {
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${escape(title)}</title>
<style>${style}</style>
${katexStyle ? `<style>${katexStyle}</style>` : ""}
</head>
<body>
${body}
</body>
</html>
`;
}
