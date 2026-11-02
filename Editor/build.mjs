// Builds the editor into one HTML file with everything inlined, committed in the app's
// resources so the app builds without Node (ADR 0004). The hash of the sources goes in the
// file: the app's build phase recomputes it and fails if the bundle is out of date. Mermaid,
// 3.5 MB, and KaTeX are copied next to it as they are published: the page loads them only for a
// diagram or a formula. KaTeX's fonts go into its CSS, in WebKit's woff2 only: the app's
// resources are one folder, without KaTeX's `fonts/`.
import { execFileSync } from "node:child_process";
import { copyFile, readFile, writeFile } from "node:fs/promises";
import { build } from "esbuild";

const output = new URL("../Noteki/Resources/editor.html", import.meta.url);
const resources = new URL("../Noteki/Resources/", import.meta.url);
await copyFile(new URL("node_modules/mermaid/dist/mermaid.min.js", import.meta.url), new URL("mermaid.min.js", resources));
await copyFile(new URL("node_modules/katex/dist/katex.min.js", import.meta.url), new URL("katex.min.js", resources));
const katexFonts = new URL("node_modules/katex/dist/fonts/", import.meta.url);
let katexStyle = await readFile(new URL("node_modules/katex/dist/katex.min.css", import.meta.url), "utf8");
for (const [source, font] of [...katexStyle.matchAll(/src:url\(fonts\/([\w-]+)\.woff2\)[^;}]*/g)].map((found) => [found[0], found[1]])) {
  const data = (await readFile(new URL(`${font}.woff2`, katexFonts))).toString("base64");
  katexStyle = katexStyle.replace(source, `src:url(data:font/woff2;base64,${data}) format("woff2")`);
}
await writeFile(new URL("katex.min.css", resources), katexStyle);
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
