/** 将已录制事实编排成紧凑索引；不读取示范计划，也不替模型推断任务。 */
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

export async function loadEvidence(directory) {
  const read = async (file) =>
    JSON.parse(await readFile(resolve(directory, file), "utf8"));
  const source = await read("model-input/evidence.json");
  const normalized = await read("model-input/normalized.json");
  const rawSamples = (
    await readFile(resolve(directory, "samples.jsonl"), "utf8")
  )
    .trim()
    .split("\n")
    .map(JSON.parse);
  if (
    !source.recording.complete ||
    source.recording.lost ||
    source.errors.length
  )
    throw Error("录制不完整");
  const origin = Math.min(...source.samples.map((s) => s.from_epoch_ms));
  const details = new Map();
  const targets = {};
  const timeline = [];
  const add = (id, time, fact, detail) => {
    details.set(id, detail);
    timeline.push({ id, ms: time - origin, ...fact });
  };
  /** 窗口和当前控件身份不冒充跨启动可复用定位器。 */
  for (const s of source.samples) {
    const id = `w${s.window.handle}`;
    targets[id] ??= {
      app: s.window.class,
      title: s.window.title,
      window_handle: s.window.handle,
      pid: s.window.pid,
    };
    if (s.window.class === "Notepad")
      targets[id].control = {
        class_name: s.focus?.target.class_name,
        automation_id: s.focus?.target.automation_id || null,
        runtime_id: s.focus?.runtime_id,
        locator_validation:
          "未记录 AutomationId 唯一性；RuntimeId 仅当前会话身份",
      };
  }
  const previous = new Map();
  for (const s of source.samples) {
    const target = `w${s.window.handle}`;
    const prior = previous.get(target);
    const state = s.focus?.text?.Available;
    const native = s.window.class === "Notepad";
    const changed =
      native &&
      JSON.stringify(state) !== JSON.stringify(prior?.focus?.text?.Available);
    const clipChanged = s.clipboard?.content !== "Unchanged";
    const visualChanged =
      s.window.title === "微信" &&
      JSON.stringify(s.ocr) !== JSON.stringify(prior?.ocr);
    const documentChanged =
      native &&
      JSON.stringify(s.documents) !== JSON.stringify(prior?.documents);
    if (!prior || changed || clipChanged || visualChanged || documentChanged) {
      add(
        s.id,
        s.from_epoch_ms,
        {
          target,
          kind: "state_observation",
          interval_ms: [s.from_epoch_ms - origin, s.through_epoch_ms - origin],
          ...(native && state
            ? {
                document_utf16_length: state.document.length,
                document_changed: prior
                  ? state.document !== prior.focus?.text?.Available?.document
                  : null,
                selections: state.selections.map((r) => ({
                  start: r.start_utf16,
                  end: r.end_utf16,
                  collapsed: r.collapsed,
                  truncated: r.truncated,
                })),
              }
            : {}),
          ...(clipChanged
            ? {
                clipboard_sequence: s.clipboard.sequence,
                clipboard_changed: true,
              }
            : {}),
          detail_available: true,
        },
        s,
      );
    }
    previous.set(target, s);
  }
  for (const e of normalized.events) {
    if (
      e.kind !== "key_down" ||
      /^(Left|Right)?(Control|Shift|Alt)$/.test(e.key)
    )
      continue;
    add(
      `key-${e.id}`,
      e.epoch_ms,
      { target: `w${e.window}`, kind: "keyboard_chord", keys: e.held },
      e,
    );
  }
  for (const e of source.cdp) {
    if (!["pointerdown", "pointerup", "copy", "paste", "cut"].includes(e.kind))
      continue;
    const s = e.selection;
    add(
      e.id,
      e.epoch_ms,
      {
        target: "browser-document",
        kind: e.kind,
        point: e.point,
        selection: {
          text: s.text,
          anchor: s.anchor && { node: s.anchor.node, offset: s.anchor.offset },
          focus: s.focus && { node: s.focus.node, offset: s.focus.offset },
        },
      },
      e,
    );
  }
  targets["browser-document"] = {
    app: "Chrome",
    css_selector: null,
    locator_validation:
      "旧录制未采集 CSS 唯一性；保留文档内节点身份，不事后猜选择器",
  };
  timeline.sort((a, b) => a.ms - b.ms || a.id.localeCompare(b.id));
  return { directory, source, rawSamples, details, origin, timeline, targets };
}

/** 检索只读原始事实，限定可见字段，不能访问磁盘上的任意文件。 */
export function inspectFacts(recording, ids) {
  if (!Array.isArray(ids) || ids.length < 1 || ids.length > 24)
    throw Error("一次请求 1–24 个证据 ID");
  return ids.map((id) => {
    const value = recording.details.get(id);
    if (!value) return { id, error: "unknown_evidence_id" };
    if (!id.startsWith("sample-")) return { id, ...value };
    return {
      id,
      interval: [value.from_epoch_ms, value.through_epoch_ms],
      window: value.window,
      target: value.focus?.target,
      runtime_id: value.focus?.runtime_id,
      text: value.focus?.text,
      documents: value.documents,
      editor_empty: value.editor_empty,
      clipboard: value.clipboard,
      clipboard_current: value.clipboard_current,
      ocr: value.ocr,
    };
  });
}
