// The messages between the app and the editor, as JSON with a `type`. Their Swift twins are in
// NotekiCore's EditorBridge.swift: a test checks both sides name the same commands and messages.

/** From the app to the editor. */
export type EditorCommand =
  /** Show this Note's text; what the editor reports is tagged with `noteID`. */
  | { type: "load"; noteID: string; text: string }
  /** Report now any change not reported yet, before the app switches Note or quits. */
  | { type: "flush" }
  /** Show nothing and take no typing: no Note is open, as after switching Vault. */
  | { type: "clear" }
  /** From the Edit menu: WebKit's own versions see only the lines on screen. */
  | { type: "selectAll" }
  | { type: "undo" }
  | { type: "redo" }
  /** The Edit menu's Find items, over the whole Note. */
  | { type: "find"; step: "open" | "next" | "previous" };

/** From the editor to the app. */
export type EditorMessage =
  | { type: "ready" }
  /** The whole text after the user typed; `revision` grows with every report. */
  | { type: "changed"; noteID: string; revision: number; text: string };

declare global {
  interface Window {
    /** Returns the reply to a `flush` as JSON, null for other commands or nothing to report. */
    noteki: { receive(command: EditorCommand): string | null };
    webkit?: { messageHandlers: { noteki: { postMessage(message: string): void } } };
  }
}

export function send(message: EditorMessage) {
  window.webkit?.messageHandlers.noteki.postMessage(JSON.stringify(message));
}
