// Builds the editor into one HTML file with everything inlined, committed in the app's
// resources so the app builds without Node (ADR 0004). The hash of the sources goes in the
// file: the app's build phase recomputes it and fails if the bundle is out of date.
import { execFileSync } from "node:child_process";
import { readFile, writeFile } from "node:fs/promises";
import { build } from "esbuild";

const output = new URL("../Noteki/Resources/editor.html", import.meta.url);
const hashScript = new URL("../scripts/editor-sources-hash", import.meta.url);

const result = await build({
  entryPoints: ["src/main.ts"],
  bundle: true,
  minify: true,
  format: "iife",
  target: "safari18",
  write: false,
  legalComments: "none",
});
const script = result.outputFiles[0].text.replaceAll("</script", "<\\/script");
const style = await readFile("src/editor.css", "utf8");
const hash = execFileSync(hashScript.pathname, { encoding: "utf8" }).trim();

await writeFile(
  output,
  `<!doctype html>
<!-- noteki-editor-sources: ${hash} -->
<html>
<head>
<meta charset="utf-8">
<style>${style}</style>
</head>
<body>
<div id="editor"></div>
<script>${script}</script>
</body>
</html>
`,
);
console.log(`editor.html: ${(script.length / 1024).toFixed(0)} KB of script, sources ${hash.slice(0, 12)}`);
