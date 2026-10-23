import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { frontmatter } from "./frontmatter";
import { wikilinks } from "./wikilink";

/** Obsidian's Markdown: GitHub's, with the Frontmatter and Wikilinks. */
export const noteLanguage = markdown({ base: markdownLanguage, extensions: [frontmatter, wikilinks] });
