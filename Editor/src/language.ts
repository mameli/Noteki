import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { codeLanguage } from "./codelanguages";
import { comments } from "./comments";
import { footnotes } from "./footnotes";
import { frontmatter } from "./frontmatter";
import { highlights } from "./highlight";
import { math } from "./math";
import { wikilinks } from "./wikilink";

/** Obsidian's Markdown: GitHub's, with the Frontmatter, Wikilinks, highlights, Comments, LaTeX and footnotes, and code in its language. */
export const noteLanguage = markdown({
  base: markdownLanguage,
  codeLanguages: codeLanguage,
  extensions: [frontmatter, comments, wikilinks, highlights, math, footnotes],
});
