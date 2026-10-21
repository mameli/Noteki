// The messages between the app and the editor, as JSON with a `type`. Their Swift twins are in
// NotekiCore's EditorBridge.swift: a test checks both sides name the same commands and messages.

/** From the app to the editor. */
export type EditorCommand =
  /** Show this Note's text; what the editor reports is tagged with `noteID`. */
  | { type: "load"; noteID: string; text: string }
  /**
   * The same Note changed on disk: its new text, the cursor kept in place, outside the undo
   * history. `revision` is the last report the app received. Replies "applied", or null when the
   * editor has anything newer: then it reports that first and the app keeps the old base.
   */
  | { type: "reload"; noteID: string; revision: number; text: string }
  /** Report now any change not reported yet, before the app switches Note or quits. */
  | { type: "flush" }
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
  /** The answer to the `completeLink` with the same `requestID`. */
  | { type: "linkCompletions"; requestID: number; completions: LinkCompletions };

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
  /** The user clicked a Wikilink: what is between its brackets. */
  | { type: "followLink"; inner: string }
  /** The Note's links, what is between their brackets: which lead nowhere? */
  | { type: "checkLinks"; noteID: string; inners: string[] }
  /** The user typed `[[`: which Notes can the link lead to? Answered by `linkCompletions`. */
  | { type: "completeLink"; requestID: number };

declare global {
  interface Window {
    /** The reply to a `flush` (a message as JSON) or a `reload` ("applied"), else null. */
    noteki: { receive(command: EditorCommand): string | null };
    webkit?: { messageHandlers: { noteki: { postMessage(message: string): void } } };
  }
}

export function send(message: EditorMessage) {
  window.webkit?.messageHandlers.noteki.postMessage(JSON.stringify(message));
}
