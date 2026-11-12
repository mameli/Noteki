/**
 * How the text is drawn, chosen in the app's Settings: a font family and the code's (none for the
 * system's), the size in points, the longest a line is, if limited, in characters of half an em (a
 * letter's width in text on average; CSS's `ch` is a digit's, a third wider). Spelling is
 * WebKit's own, set by the app.
 */
export interface EditorSettings {
  /** The theme's name in editor.css, `data-theme` on the page. */
  theme: string;
  font?: string;
  monoFont?: string;
  size: number;
  lineLength: number;
  limitsLineLength: boolean;
  /** View → Focus Mode: all but the paragraph being written faded (writing.ts). */
  focusMode?: boolean;
  /** View → Typewriter Mode: the line being written kept in the middle of the window. */
  typewriterMode?: boolean;
}

/** A family's name in quotes it cannot break out of, before the fallback. */
function family(name: string | undefined, fallback: string): string {
  return name ? `"${name.replaceAll("\\", "\\\\").replaceAll('"', '\\"')}", ${fallback}` : fallback;
}

/** The CSS variables the editor's text is drawn with. */
export function settingsVariables(settings: EditorSettings): Record<string, string> {
  return {
    "--text-font": family(settings.font, "var(--system-font)"),
    "--mono": family(settings.monoFont, "var(--system-mono)"),
    "--text-size": `${settings.size}px`,
    "--line-width": settings.limitsLineLength ? `calc(${settings.lineLength} * 0.5 * var(--text-size))` : "none",
  };
}

/** Sent on the window when the theme may have changed. */
export const coloursChanged = "noteki-colours";

/** Sets them on the page: the app gives its settings before the page loads, and again when they change. */
export function applySettings(settings: EditorSettings) {
  document.documentElement.dataset.theme = settings.theme;
  for (const [name, value] of Object.entries(settingsVariables(settings))) {
    document.documentElement.style.setProperty(name, value);
  }
  // What draws with the colours read once (an SVG) draws again.
  window.dispatchEvent(new Event(coloursChanged));
}
