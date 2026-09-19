/** 只对录制数据解码、排序和比较；业务意图不进入事实时间线。 */
const keyNames = new Map([
  [13, "Enter"],
  [16, "Shift"],
  [17, "Ctrl"],
  [18, "Alt"],
  [35, "End"],
  [36, "Home"],
  [37, "←"],
  [38, "↑"],
  [39, "→"],
  [40, "↓"],
  [160, "Shift"],
  [161, "Shift"],
  [162, "Ctrl"],
  [163, "Ctrl"],
  [164, "Alt"],
  [165, "Alt"],
]);
export const keyName = (vk) =>
  keyNames.get(vk) ??
  (vk >= 65 && vk <= 90 ? String.fromCharCode(vk) : `VK ${vk}`);

export function buildEvents({ session, raw, samples, cdp }) {
  const epoch = (qpc) =>
    session.epoch_ms + ((qpc - session.qpc) * 1000) / session.frequency;
  const rows = [],
    held = new Set();
  const append = (value) =>
    rows.push({ ...value, ms: Math.round(value.time - session.epoch_ms) });
  for (const [line, e] of raw.entries()) {
    const key = e.kind?.Key;
    let title = typeof e.kind === "string" ? e.kind : Object.keys(e.kind)[0];
    let important = true;
    if (e.kind === "Context") {
      held.clear();
      title = "前台窗口上下文变化";
    }
    if (key) {
      const name = keyName(key.vk);
      if (key.down) held.add(name);
      title = `${key.down ? "按下" : "释放"} ${key.down ? [...held].join(" + ") : name}`;
      important = key.down && !["Ctrl", "Shift", "Alt"].includes(name);
      if (!key.down) held.delete(name);
    }
    append({
      id: `key-${e.sequence}`,
      time: epoch(e.qpc),
      type: "input",
      title,
      app: `PID ${e.window.pid}`,
      window: e.window.handle,
      important,
      raw: e,
      file: "events.jsonl",
      line: line + 1,
      origin: e.origin,
      note: "键名由 VK 解码；按键已触发，不代表业务结果成功。",
    });
  }
  const previous = new Map();
  for (const [line, s] of samples.entries()) {
    const prev = previous.get(s.window?.handle),
      state = s.focus?.text?.Available,
      old = prev?.focus?.text?.Available;
    const docChange = !!state && !!old && state.document !== old.document;
    const selectionChange =
      !!state &&
      !!old &&
      JSON.stringify(state.selections) !== JSON.stringify(old.selections);
    let title = "焦点与画面快照";
    if (docChange)
      title = `文档变化 · ${old.document.length} → ${state.document.length} UTF-16`;
    else if (selectionChange)
      title = `选区变化 · ${state.selections.map((r) => `[${r.start_utf16 ?? "?"}, ${r.end_utf16 ?? "?"})`).join(" / ")}`;
    append({
      id: `sample-${s.id}`,
      time: epoch(s.from_qpc),
      end: epoch(s.through_qpc),
      type: "snapshot",
      title,
      app: s.window?.class,
      window: s.window?.handle,
      important: !prev || docChange || selectionChange,
      raw: s,
      file: "samples.jsonl",
      line: line + 1,
      sample: s.id,
      before: prev?.id,
      note: "采样期间依次取得截图、UIA、OCR 和剪贴板；不能当作同一瞬间。",
    });
    if (s.clipboard_observation?.content !== "Unchanged")
      append({
        id: `clipboard-${s.id}`,
        time: epoch(s.through_qpc),
        type: "clipboard",
        title: "剪贴板序号变化",
        app: s.window?.class,
        window: s.window?.handle,
        important: true,
        raw: s.clipboard_observation,
        file: "samples.jsonl",
        line: line + 1,
        sample: s.id,
        note: "来自 clipboard_observation 字段；这是采样区间结束时间，不是剪贴板精确变更时刻。",
      });
    previous.set(s.window?.handle, s);
  }
  for (const [line, batch] of cdp.entries())
    for (const e of batch.observation?.events ?? [])
      append({
        id: `cdp-${line}-${e.sequence}`,
        time: batch.time_origin + e.time,
        type: "cdp",
        title:
          {
            pointerdown: "浏览器鼠标按下",
            pointerup: "浏览器鼠标释放",
            selectionchange: "浏览器选区变化",
            copy: "浏览器触发 copy 事件",
          }[e.kind] ?? e.kind,
        app: "Chrome",
        important: e.kind !== "selectionchange",
        raw: e,
        file: "cdp.jsonl",
        line: line + 1,
        batch: {
          url: batch.url,
          time_origin: batch.time_origin,
          from_epoch_ms: batch.from_epoch_ms,
          through_epoch_ms: batch.through_epoch_ms,
        },
        note: "页面事件监听器经 CDP 读出；trusted=true 也不能证明是人工操作，CDP 注入可产生可信事件。",
      });
  return rows.sort((a, b) => a.time - b.time || a.id.localeCompare(b.id));
}
