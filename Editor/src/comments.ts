import { styleTags, tags } from "@lezer/highlight";
import type { BlockContext, Element, Line, MarkdownConfig } from "@lezer/markdown";

// Comments, `%%…%%`, as Obsidian writes them: part of the Note, hidden while reading. Inside a
// paragraph a Comment ends at the next `%%` of that paragraph, on its line or a later one; a
// line starting with `%%` and not closing it, outside lists and quotes, opens a Comment that runs
// over blank lines and blocks to the next line holding `%%`, or to the end of the Note. Nothing
// inside is Markdown.
const percent = 37; // %

/**
 * Whether a line opens a Comment of lines: `%%` first and not closed on it, outside lists and
 * quotes, where the lines after would have to belong to them (a paragraph's Comment instead).
 */
function opens(cx: BlockContext, line: Line): boolean {
  if (line.next !== percent || line.text.charCodeAt(line.pos + 1) !== percent) return false;
  if (line.indent >= 4 || line.text.includes("%%", line.pos + 2)) return false;
  // Right under a list item or a quote, a line at the margin ends them, as it would end a paragraph.
  return cx.parentType().name === "Document" || line.basePos === 0;
}

export const comments: MarkdownConfig = {
  defineNodes: [{ name: "CommentBlock", block: true }, "Comment", "CommentMark"],
  props: [styleTags({ "Comment CommentBlock": tags.comment, CommentMark: tags.processingInstruction })],
  parseBlock: [
    {
      name: "CommentBlock",
      before: "FencedCode",
      // Right under a paragraph's line too, ending it, unless it closes a Comment the paragraph opened.
      endLeaf: (cx, line, leaf) => opens(cx, line) && leaf.content.split("%%").length % 2 === 1,
      parse(cx, line) {
        if (!opens(cx, line)) return false;
        const from = cx.lineStart + line.pos;
        const marks: Element[] = [cx.elt("CommentMark", from, from + 2)];
        let end = cx.lineStart + line.text.length;
        while (cx.nextLine()) {
          end = cx.lineStart + line.text.length;
          const close = line.text.indexOf("%%", line.pos);
          if (close !== -1) {
            marks.push(cx.elt("CommentMark", cx.lineStart + close, cx.lineStart + close + 2));
            cx.nextLine();
            break;
          }
        }
        cx.addElement(cx.elt("CommentBlock", from, end, marks));
        return true;
      },
    },
  ],
  parseInline: [
    {
      name: "Comment",
      before: "InlineCode",
      parse(cx, next, pos) {
        if (next !== percent || cx.char(pos + 1) !== percent) return -1;
        const close = cx.slice(pos + 2, cx.end).indexOf("%%");
        if (close === -1) return -1;
        const end = pos + 2 + close + 2;
        return cx.addElement(cx.elt("Comment", pos, end, [cx.elt("CommentMark", pos, pos + 2), cx.elt("CommentMark", end - 2, end)]));
      },
    },
  ],
};
