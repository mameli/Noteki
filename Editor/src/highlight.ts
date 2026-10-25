import { styleTags, Tag, tags } from "@lezer/highlight";
import type { MarkdownConfig } from "@lezer/markdown";

// `==highlight==`, as Obsidian writes it, parsed as GitHub's `~~strikethrough~~` is: the marks
// open and close by the same rules, so `a == b` is no highlight.
const equals = 61; // =

const delimiter = { resolve: "Highlight", mark: "HighlightMark" };
const punctuation = /[\p{P}\p{S}]/u;

/** The highlighted text, drawn with a background. */
export const highlightTag = Tag.define();

export const highlights: MarkdownConfig = {
  defineNodes: ["Highlight", "HighlightMark"],
  props: [styleTags({ "Highlight/...": highlightTag, HighlightMark: tags.processingInstruction })],
  parseInline: [
    {
      name: "Highlight",
      after: "Emphasis",
      parse(cx, next, pos) {
        if (next !== equals || cx.char(pos + 1) !== equals || cx.char(pos + 2) === equals || cx.char(pos - 1) === equals) return -1;
        const before = cx.slice(pos - 1, pos);
        const after = cx.slice(pos + 2, pos + 3);
        const spaceBefore = /\s|^$/.test(before);
        const spaceAfter = /\s|^$/.test(after);
        const punctuationBefore = punctuation.test(before);
        const punctuationAfter = punctuation.test(after);
        return cx.addDelimiter(
          delimiter,
          pos,
          pos + 2,
          !spaceAfter && (!punctuationAfter || spaceBefore || punctuationBefore),
          !spaceBefore && (!punctuationBefore || spaceAfter || punctuationAfter),
        );
      },
    },
  ],
};
