import type { EditorMessage } from "../../Editor/src/bridge";

type Listener = (message: EditorMessage) => void;

let listener: Listener | null = null;
const queued: EditorMessage[] = [];

export function setEditorMessageHandler(handler: Listener) {
  listener = handler;
  while (queued.length > 0) {
    const next = queued.shift()!;
    handler(next);
  }
}

// Provide the settings & webkit bridge expected by Editor/src/main.ts BEFORE it runs.
window.notekiSettings = {
  theme: "ayu",
  size: 15,
  lineLength: 100,
  limitsLineLength: true,
  focusMode: false,
  typewriterMode: false,
};

window.webkit = {
  messageHandlers: {
    noteki: {
      postMessage(raw: string) {
        const msg = JSON.parse(raw) as EditorMessage;
        if (listener) listener(msg);
        else queued.push(msg);
      },
    },
  },
};
