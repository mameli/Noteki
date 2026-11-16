import { copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const esbuild = require(path.resolve("../Editor/node_modules/esbuild/lib/main.js"));

const distDir = path.resolve("dist");
await mkdir(distDir, { recursive: true });

// Copy KaTeX & Mermaid assets so Editor/src/math.ts and Editor/src/mermaid.ts load them on demand.
await copyFile(path.resolve("../Noteki/Resources/mermaid.min.js"), path.join(distDir, "mermaid.min.js"));
await copyFile(path.resolve("../Noteki/Resources/katex.min.js"), path.join(distDir, "katex.min.js"));
await copyFile(path.resolve("../Noteki/Resources/katex.min.css"), path.join(distDir, "katex.min.css"));

await copyFile(path.resolve("index.html"), path.join(distDir, "index.html"));

await esbuild.build({
  entryPoints: ["src/app.ts"],
  bundle: true,
  minify: false,
  sourcemap: true,
  format: "esm",
  target: ["safari18", "chrome110", "firefox115"],
  outfile: path.join(distDir, "app.js"),
  nodePaths: [path.resolve("../Editor/node_modules"), path.resolve("node_modules")],
  loader: { ".css": "css" },
});

console.log("NotekiTauri frontend built to dist/");
