import { readFile, writeFile, readdir } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createRequire } from "node:module";
import { compile } from "tailwindcss";
import { loadRecording } from "./load.mjs";
import { embedFrames } from "./media.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "../../../../..");
const directory = resolve(
  process.argv[2] ?? resolve(here, "../output/model-03"),
);
const require = createRequire(import.meta.url);
const viteRequire = createRequire(require.resolve("vite/package.json"));
const { build } = await import(
  pathToFileURL(viteRequire.resolve("esbuild")).href
);
const data = await loadRecording(root, directory);
data.images = await embedFrames(directory, data.samples, here);
const sources = await Promise.all(
  (await readdir(here))
    .filter((file) => /\.(mjs|html)$/.test(file))
    .map((file) => readFile(resolve(here, file), "utf8")),
);
const candidates = [...new Set(sources.join("\n").match(/[^\s"'`<>]+/g))];
const compiler = await compile(
  await readFile(require.resolve("tailwindcss/index.css"), "utf8"),
);
const css = compiler.build(candidates);
const bundle = await build({
  entryPoints: [resolve(here, "ui.mjs")],
  bundle: true,
  write: false,
  platform: "browser",
  format: "iife",
  minify: true,
  legalComments: "none",
});
const json = JSON.stringify(data)
  .replace(/</g, "\\u003c")
  .replace(/\u2028/g, "\\u2028")
  .replace(/\u2029/g, "\\u2029");
const html = (await readFile(resolve(here, "template.html"), "utf8"))
  .replace("/* GENERATED_STYLE */", () => css)
  .replace("/* GENERATED_DATA */", () => json)
  .replace("/* GENERATED_SCRIPT */", () => bundle.outputFiles[0].text);
const output = resolve(directory, "recording-viewer.html");
await writeFile(output, html);
console.log(
  JSON.stringify({
    output,
    bytes: Buffer.byteLength(html),
    records: data.records.length,
    images: Object.keys(data.images).length,
    counts: data.counts,
  }),
);
