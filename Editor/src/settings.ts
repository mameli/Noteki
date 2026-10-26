/** How the text is drawn, chosen in the app's Settings: a font family (none for the system's), its size and the widest a line is, in points. */
export interface EditorSettings {
  font?: string;
  size: number;
  lineWidth: number;
}

/** The CSS variables the editor's text is drawn with. */
export function settingsVariables(settings: EditorSettings): Record<string, string> {
  const font = settings.font ? `"${settings.font.replaceAll("\\", "\\\\").replaceAll('"', '\\"')}", var(--system-font)` : "var(--system-font)";
  return {
    "--text-font": font,
    "--text-size": `${settings.size}px`,
    "--line-width": `${settings.lineWidth}px`,
  };
}

/** Sets them on the page: the app gives its settings before the page loads, and again when they change. */
export function applySettings(settings: EditorSettings) {
  for (const [name, value] of Object.entries(settingsVariables(settings))) {
    document.documentElement.style.setProperty(name, value);
  }
}
