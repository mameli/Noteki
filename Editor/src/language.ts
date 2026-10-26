import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { codeLanguage } from "./codelanguages";
import { comments } from "./comments";
import { frontmatter } from "./frontmatter";
import { highlights } from "./highlight";
import { wikilinks } from "./wikilink";

/** Obsidian's Markdown: GitHub's, with the Frontmatter, Wikilinks, highlights and Comments, and code in its language. */
export const noteLanguage = markdown({
  base: markdownLanguage,
  codeLanguages: codeLanguage,
  extensions: [frontmatter, comments, wikilinks, highlights],
});
