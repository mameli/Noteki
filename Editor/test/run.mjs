// Runs the editor's tests (Seam 2: CodeMirror's state with Noteki's extensions) under Node:
// each test/*.test.ts is bundled with esbuild, as the editor is, then run by node --test.
import { readdir, rm } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { build } from "esbuild";

const out = new URL("./.out/", import.meta.url).pathname;
await rm(out, { recursive: true, force: true });
const tests = (await readdir(new URL(".", import.meta.url))).filter((name) => name.endsWith(".test.ts"));
await build({
  entryPoints: tests.map((name) => new URL(name, import.meta.url).pathname),
  bundle: true,
  format: "esm",
  platform: "node",
  outdir: out,
  outExtension: { ".js": ".mjs" },
  logLevel: "warning",
});
const result = spawnSync(process.execPath, ["--test", out], { stdio: "inherit" });
process.exit(result.status ?? 1);
