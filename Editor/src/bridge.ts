import type { EmbeddedFile } from "./embeds";
import type { FrontmatterReading } from "./frontmatter";
import type { EditorSettings } from "./settings";

// The messages between the app and the editor, as JSON with a `type`. Their Swift twins are in
// NotekiCore's EditorBridge.swift: a test checks both sides name the same commands and messages.

/** From the app to the editor. */
export type EditorCommand =
  /**
   * Show this Note's text; what the editor reports is tagged with `noteID`. `line`, from 1, is
   * where the cursor goes and the view scrolls to: a Backlink opens its Note at the link.
   */
  | { type: "load"; noteID: string; text: string; line?: number }
  /**
   * The same Note changed on disk: its new text, the cursor kept in place, outside the undo
   * history. `revision` is the last report the app received. Replies "applied", or null when the
   * editor has anything newer: then it reports that first and the app keeps the old base.
   */
  | { type: "reload"; noteID: string; revision: number; text: string }
  /** Report now any change not reported yet, before the app switches Note or quits. */
  | { type: "flush" }
  /**
   * A Tab back in front: the state its Note had in the editor when the Tab was left (cursor,
   * scroll, undo), with `text` as the app has it now; a new one if none was kept.
   */
  | { type: "restore"; noteID: string; text: string }
  /** The Tabs of these Notes closed, or showing another Note: their states are not kept. */
  | { type: "drop"; noteIDs: string[] }
  /** Show nothing and take no typing: no Note is open, as after switching Vault. */
  | { type: "clear" }
  /** From the Edit menu: WebKit's own versions see only the lines on screen. */
  | { type: "selectAll" }
  | { type: "undo" }
  | { type: "redo" }
  /** The Edit menu's Find items, over the whole Note. */
  | { type: "find"; step: "open" | "next" | "previous" }
  /** Which of the Note's links lead nowhere: those are drawn as Unresolved links. */
  | { type: "unresolvedLinks"; noteID: string; inners: string[] }
  /** The files the Note's Embeds and Markdown images show, by what the editor asked about. */
  | { type: "embeds"; noteID: string; files: Record<string, EmbeddedFile> }
  /** Source mode: the Markdown as written, every mark shown. Off, Live preview. */
  | { type: "sourceMode"; on: boolean }
  /** The cursor at the start of `line`, from 1, in the middle of the view: a search result. */
  | { type: "goToLine"; noteID: string; line: number }
  /** The answer to the `completeLink` with the same `requestID`. */
  | { type: "linkCompletions"; requestID: number; completions: LinkCompletions }
  /**
   * The answer to `readFrontmatter`: the `text` it asked about and its reading, none when the
   * Frontmatter never closes. The table is drawn only while the Frontmatter is still that text.
   */
  | { type: "frontmatter"; noteID: string; text: string; reading?: FrontmatterReading }
  /**
   * The answer to `addTag` or `removeTag`: what to put in place of the Frontmatter `text`, only
   * while it is still that text.
   */
  | { type: "replaceFrontmatter"; noteID: string; text: string; replacement: string }
  /** The answer to the `completeTag` with the same `requestID`: the Vault's Tags. */
  | { type: "tagCompletions"; requestID: number; tags: string[] }
  /**
   * Text written for the user, as typed: the Embeds of a Pasted image in place of the selection,
   * or of dropped files at `point`, where they were dropped (CSS pixels from the top left).
   */
  | { type: "insert"; noteID: string; text: string; point?: { x: number; y: number } }
  /** The text's font, size and line width changed in Settings. */
  | { type: "settings"; settings: EditorSettings };

/** What `[[` offers: the Notes opened recently, the latest first, then all the others. */
export interface LinkCompletions {
  recent: LinkCompletion[];
  others: LinkCompletion[];
}

export interface LinkCompletion {
  /** What the user's typing is matched against. */
  name: string;
  /** Where the Note is, shown beside its name; empty at the Vault's root. */
  folder: string;
  /** What is written between the brackets when the Note is chosen. */
  link: string;
}

/** From the editor to the app. */
export type EditorMessage =
  | { type: "ready" }
  /** The whole text after the user typed; `revision` grows with every report. */
  | { type: "changed"; noteID: string; revision: number; text: string }
  /** The user clicked a Wikilink: what is between its brackets; with ⌘, for a new Tab. */
  | { type: "followLink"; inner: string; newTab: boolean }
  /** The user clicked a link to a web address (http, https, mailto): for the browser. */
  | { type: "openURL"; url: string }
  /** The Note's links, what is between their brackets: which lead nowhere? */
  | { type: "checkLinks"; noteID: string; inners: string[] }
  /** The user typed `[[`: which Notes can the link lead to? Answered by `linkCompletions`. */
  | { type: "completeLink"; requestID: number }
  /** The Frontmatter, from its first `---` line to its last: how does it read? */
  | { type: "readFrontmatter"; noteID: string; text: string }
  /** A Tag added in the Properties table: `text` is the Frontmatter. */
  | { type: "addTag"; noteID: string; text: string; tag: string }
  /** A chip's ×: remove `tag` from the Frontmatter `text`. */
  | { type: "removeTag"; noteID: string; text: string; tag: string }
  /** The user is adding a Tag: which Tags does the Vault have? */
  | { type: "completeTag"; requestID: number };

declare global {
  interface Window {
    /** The reply to a `flush` (a message as JSON) or a `reload` ("applied"), else null. */
    noteki: { receive(command: EditorCommand): string | null };
    /** Set by the app before the page loads (ADR 0004): the text is drawn right from the start. */
    notekiSettings?: EditorSettings;
    webkit?: { messageHandlers: { noteki: { postMessage(message: string): void } } };
  }
}

export function send(message: EditorMessage) {
  window.webkit?.messageHandlers.noteki.postMessage(JSON.stringify(message));
}
