import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { comments } from "./comments";
import { frontmatter } from "./frontmatter";
import { highlights } from "./highlight";
import { wikilinks } from "./wikilink";

/** Obsidian's Markdown: GitHub's, with the Frontmatter, Wikilinks, highlights and Comments. */
export const noteLanguage = markdown({ base: markdownLanguage, extensions: [frontmatter, comments, wikilinks, highlights] });
