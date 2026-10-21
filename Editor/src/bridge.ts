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
  | { type: "unresolvedLinks"; noteID: string; inners: string[] };

/** From the editor to the app. */
export type EditorMessage =
  | { type: "ready" }
  /** The whole text after the user typed; `revision` grows with every report. */
  | { type: "changed"; noteID: string; revision: number; text: string }
  /** The user clicked a Wikilink: what is between its brackets. */
  | { type: "followLink"; inner: string }
  /** The Note's links, what is between their brackets: which lead nowhere? */
  | { type: "checkLinks"; noteID: string; inners: string[] };

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
