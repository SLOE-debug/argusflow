/** 显式导入指定历史录制的原始文件；不读取当时推导的流程或示范计划。 */
import { readFile, writeFile, mkdir } from "node:fs/promises";
import { resolve } from "node:path";
const directory = resolve(process.argv[2]);
const read = async (name) =>
  JSON.parse(await readFile(resolve(directory, name), "utf8"));
const lines = async (name) =>
  (await readFile(resolve(directory, name), "utf8"))
    .trim()
    .split("\n")
    .filter(Boolean)
    .map(JSON.parse);
const session = await read("session.json"),
  finished = await read("finished.json");
if (!finished.complete || finished.lost) throw Error("录制不完整");
const epoch = (qpc) =>
  Math.round(
    session.epoch_ms + ((qpc - session.qpc) * 1000) / session.frequency,
  );
const raw = await lines("samples.jsonl");
const samples = raw
  .filter((s) => s.window && !s.error)
  .map((s) => ({
    id: `sample-${s.id}`,
    from_epoch_ms: epoch(s.from_qpc),
    through_epoch_ms: epoch(s.through_qpc),
    frame_through_epoch_ms: epoch(s.frame_through_qpc),
    window: s.window,
    focus: s.focus,
    documents: s.documents,
    clipboard: {
      sequence: s.clipboard_sequence,
      content:
        s.clipboard == null
          ? "Unavailable"
          : { Text: { text: s.clipboard, truncated: false } },
    },
    clipboard_current: s.clipboard,
    ocr: s.ocr,
    editor_empty: s.editor_empty,
    image: s.image,
    crop: [0, 0, s.bounds[2], s.bounds[3]],
  }));
const handles = new Set(samples.map((s) => s.window.handle));
const events = (await lines("events.jsonl"))
  .filter((e) => handles.has(e.window.handle))
  .map((e) => ({
    id: e.sequence,
    epoch_ms: epoch(e.qpc),
    window: e.window.handle,
    point: e.point,
    origin: e.origin,
    kind: e.kind,
  }));
const cdp = (await lines("browser.jsonl")).map((e, i) => ({
  id: `cdp-archived-${i}`,
  epoch_ms: e.epoch_ms,
  kind: e.type,
  point: e.x === null ? null : [e.x, e.y],
  trusted: e.trusted,
  selection: {
    text: e.selection,
    anchor: { node: e.xpath, offset: e.anchor_offset },
    focus: { node: e.xpath, offset: e.focus_offset },
  },
  target: { xpath: e.xpath, text: e.node_text, bounds: e.rect },
  url: e.url,
  title: e.title,
}));
const start = cdp[0].epoch_ms;
let sequence;
for (const sample of samples) {
  if (sequence === sample.clipboard.sequence)
    sample.clipboard.content = "Unchanged";
  sequence = sample.clipboard.sequence;
}
const evidence = {
  schema: "observed-evidence-v1",
  recording: finished,
  notes: [
    "显式导入历史原始录制，未读取推导结果或示范计划。历史采样未提供原生 UTF-16 选区，不补造。",
    "回放范围从第一个浏览器指针事件开始；前置启动交给宿主，范围外事件和错误保留在 prelude。",
  ],
  range_start_epoch_ms: start,
  prelude: {
    events: events.filter((e) => e.epoch_ms < start),
    errors: raw.filter((s) => s.error && epoch(s.through_qpc) < start),
  },
  errors: raw.filter((s) => s.error && epoch(s.through_qpc) >= start),
  samples: samples.filter((s) => s.through_epoch_ms >= start),
  events: events.filter((e) => e.epoch_ms >= start),
  cdp,
};
await mkdir(resolve(directory, "model-input"), { recursive: true });
await writeFile(
  resolve(directory, "model-input/evidence.json"),
  JSON.stringify(evidence, null, 2),
);
console.log(
  JSON.stringify({
    samples: samples.length,
    events: events.length,
    cdp: cdp.length,
    errors: evidence.errors.length,
  }),
);
