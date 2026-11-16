# 01: Selected text stays visible over inline code and highlights

**What to build:** The selection is painted under the text and over its backgrounds, so that selected inline code, highlights and table cells show as selected (feedback: "Quando seleziono un codice ... è praticamente invisibile").

**Blocked by:** none

**Status:** ready-for-agent

- [x] Text backgrounds (inline code, highlights) painted first, then the selection, then the text
- [x] The same in table cells

## Comments

Done (2026-11-17). The element paints in two passes over the lines on screen: first every background (code boxes, rules, quote bars, grids, inline code, highlights, table cells' backgrounds), then the selection, then images and text. Code blocks had the same fault as inline code: their box covered the selection.
