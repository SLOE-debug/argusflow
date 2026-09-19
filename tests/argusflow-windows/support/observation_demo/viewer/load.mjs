/** 固定白名单读取录制产物及代码快照，不读 .env 或用户目录。 */
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { createHash } from "node:crypto";
import { buildEvents } from "./events.mjs";
import { capabilities, presets, sourcePaths } from "./provenance.mjs";

export async function loadRecording(root, directory) {
  const manifest = [];
  const read = async (file, jsonl = false) => {
    const bytes = await readFile(resolve(directory, file));
    manifest.push({
      file,
      bytes: bytes.length,
      sha256: createHash("sha256").update(bytes).digest("hex"),
    });
    return jsonl
      ? bytes
          .toString("utf8")
          .trim()
          .split("\n")
          .filter(Boolean)
          .map(JSON.parse)
      : JSON.parse(bytes);
  };
  const session = await read("session.json"),
    finished = await read("finished.json");
  const raw = await read("events.jsonl", true),
    samples = await read("samples.jsonl", true),
    cdp = await read("cdp.jsonl", true),
    interactions = await read("interactions.jsonl", true);
  const plan = await read("actor-private.json");
  const models = [];
  for (const name of [
    "compact-single-01",
    "compact-multi-02",
    "plus-multi-01",
    "max-multi-01",
  ])
    models.push({
      name,
      workflow: await read(`${name}/workflow.json`),
      metrics: await read(`${name}/metrics.json`),
    });
  const audit = await read("conversation-audit.json");
  const code = {};
  for (const [id, file] of Object.entries(sourcePaths)) {
    const source = await readFile(resolve(root, file), "utf8");
    code[id] = {
      file,
      source,
      sha256: createHash("sha256").update(source).digest("hex"),
    };
  }
  const records = buildEvents({ session, raw, samples, cdp });
  return {
    schema: "recording-viewer-v1",
    session,
    finished,
    records,
    samples,
    interactions,
    plan,
    models,
    audit,
    manifest,
    capabilities,
    presets,
    code,
    counts: {
      input: raw.length,
      cdp: cdp.reduce((n, b) => n + (b.observation?.events.length ?? 0), 0),
      samples: samples.length,
      frames: new Set(samples.map((s) => s.image)).size,
      clipboard: records.filter((r) => r.type === "clipboard").length,
    },
    sourceDirectory: directory,
    generatedAt: new Date().toISOString(),
  };
}
