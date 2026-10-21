// The messages between the app and the editor. Their Swift twins are in the app's
// EditorBridge.swift: a change here needs the same change there.

/** From the app to the editor. */
export type EditorCommand = { type: "load"; text: string };

/** From the editor to the app. */
export type EditorMessage = { type: "ready" };

declare global {
  interface Window {
    noteki: { receive(command: EditorCommand): void };
    webkit?: { messageHandlers: { noteki: { postMessage(message: string): void } } };
  }
}

export function send(message: EditorMessage) {
  window.webkit?.messageHandlers.noteki.postMessage(JSON.stringify(message));
}
