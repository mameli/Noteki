import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { codeLanguage } from "./codelanguages";
import { comments } from "./comments";
import { frontmatter } from "./frontmatter";
import { highlights } from "./highlight";
import { math } from "./math";
import { wikilinks } from "./wikilink";

/** Obsidian's Markdown: GitHub's, with the Frontmatter, Wikilinks, highlights, Comments and LaTeX, and code in its language. */
export const noteLanguage = markdown({
  base: markdownLanguage,
  codeLanguages: codeLanguage,
  extensions: [frontmatter, comments, wikilinks, highlights, math],
});
