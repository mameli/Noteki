import type { MarkdownConfig } from "@lezer/markdown";
import { styleTags, tags } from "@lezer/highlight";

// The YAML block at the very top of a Note, between `---` lines, as Obsidian reads it.
// Without this, `tags: x` followed by `---` parses as a Markdown heading. The parser sees one
// line at a time and cannot go back: a Note that opens with `---` and never closes it is all
// Frontmatter, a rare case accepted for now.
const fence = /^---[ \t]*$/;
const closing = /^(---|\.\.\.)[ \t]*$/;

export const frontmatter: MarkdownConfig = {
  defineNodes: [{ name: "Frontmatter", block: true }],
  props: [styleTags({ Frontmatter: tags.meta })],
  parseBlock: [
    {
      name: "Frontmatter",
      before: "HorizontalRule",
      parse(cx, line) {
        if (cx.lineStart !== 0 || !fence.test(line.text)) return false;
        let end = line.text.length;
        while (cx.nextLine()) {
          end = cx.lineStart + line.text.length;
          if (closing.test(line.text)) {
            cx.nextLine();
            break;
          }
        }
        cx.addElement(cx.elt("Frontmatter", 0, end));
        return true;
      },
    },
  ],
};
