import {
  el,
  pill,
  jsonBlock,
  timeLabel,
  typeLabel,
  sourceButtons,
} from "./dom.mjs";

/** 可见选区由真实 UTF-16 偏移绘制；行号明确属于展示计算。 */
function documentView(state) {
  const box = el("div", "rounded-xl border border-stone-200 p-4");
  box.append(el("h3", "mb-3 text-sm font-semibold", "UIA 文档与选区"));
  if (!state) {
    box.append(
      el(
        "p",
        "text-sm text-stone-500",
        "本条证据没有可用的 TextPattern 文档。",
      ),
    );
    return box;
  }
  const selected = state.selections.filter(
    (r) => !r.collapsed && r.start_utf16 != null && r.end_utf16 != null,
  );
  const pre = el(
    "pre",
    "overflow-auto whitespace-pre-wrap rounded-lg bg-stone-50 p-4 font-mono text-sm leading-7",
  );
  let offset = 0;
  const ranges = selected
    .map((r) => [r.start_utf16, r.end_utf16])
    .sort((a, b) => a[0] - b[0]);
  for (const [start, end] of ranges) {
    if (start < offset || end > state.document.length) continue;
    pre.append(
      document.createTextNode(state.document.slice(offset, start)),
      el(
        "mark",
        "rounded-sm bg-teal-200 text-teal-950",
        state.document.slice(start, end),
      ),
    );
    offset = end;
  }
  pre.append(document.createTextNode(state.document.slice(offset)));
  box.append(pre);
  for (const r of state.selections) {
    const line =
      r.start_utf16 == null
        ? null
        : state.document.slice(0, r.start_utf16).split(/\r\n|\r|\n/).length;
    box.append(
      el(
        "p",
        "mt-3 text-sm text-stone-600",
        `${r.collapsed ? "折叠光标" : "非折叠选区"} [${r.start_utf16 ?? "未知"}, ${r.end_utf16 ?? "未知"}) · ${line ? "第 " + line + " 行（由换行计算）" : "行号未知"}`,
      ),
    );
    if (r.text)
      box.append(
        el(
          "p",
          "mt-1 whitespace-pre-wrap text-sm",
          `选中文字：${JSON.stringify(r.text)}`,
        ),
      );
  }
  return box;
}

function framesFor(record, data) {
  const epoch = (qpc) =>
    data.session.epoch_ms +
    ((qpc - data.session.qpc) * 1000) / data.session.frequency;
  if (record.sample !== undefined) {
    const current = data.samples.find((s) => s.id === record.sample);
    const previous = data.samples
      .filter(
        (s) => s.id < current.id && s.window.handle === current.window.handle,
      )
      .at(-1);
    return [previous, current];
  }
  const same = data.samples.filter((s) =>
    record.type === "cdp"
      ? s.window?.class === "Chrome_WidgetWin_1"
      : s.window?.handle === record.window,
  );
  return [
    same.filter((s) => epoch(s.frame_through_qpc) <= record.time).at(-1),
    same.find((s) => epoch(s.from_qpc) >= record.time),
  ];
}

function frameView(sample, data, label, overlay) {
  const figure = el(
    "figure",
    "min-w-0 rounded-xl border border-stone-200 bg-stone-50 p-3",
  );
  if (!sample) {
    figure.append(
      el(
        "p",
        "p-6 text-sm text-stone-500",
        `${label}：没有对应窗口的相邻采样帧`,
      ),
    );
    return figure;
  }
  const end =
    data.session.epoch_ms +
    ((sample.frame_through_qpc - data.session.qpc) * 1000) /
      data.session.frequency;
  figure.append(
    el(
      "figcaption",
      "mb-2 text-xs text-stone-500",
      `${label} · sample-${sample.id} · 截图完成 ${timeLabel(end - data.session.epoch_ms)}`,
    ),
  );
  const wrap = el("div", "relative overflow-hidden rounded-lg bg-stone-200");
  const img = el("img", "block h-auto w-full");
  img.src = data.images[sample.image];
  img.alt = `${sample.window.title} 的录制截图，sample-${sample.id}`;
  img.title = "点击在当前查看器内放大";
  img.tabIndex = 0;
  const zoom = () => {
    const dialog = document.querySelector("#image-dialog");
    dialog.querySelector("img").src = img.src;
    dialog.querySelector("h2").textContent = img.alt;
    dialog.showModal();
  };
  img.addEventListener("click", zoom);
  img.addEventListener("keydown", (e) => {
    if (e.key === "Enter") zoom();
  });
  wrap.append(img);
  if (overlay) {
    const ns = "http://www.w3.org/2000/svg",
      svg = document.createElementNS(ns, "svg");
    svg.setAttribute("viewBox", `0 0 ${sample.bounds[2]} ${sample.bounds[3]}`);
    svg.setAttribute(
      "class",
      "pointer-events-none absolute inset-0 h-full w-full",
    );
    for (const b of sample.ocr) {
      const r = document.createElementNS(ns, "rect");
      for (const [name, value] of Object.entries({
        x: b.rect[0],
        y: b.rect[1],
        width: b.rect[2],
        height: b.rect[3],
        fill: "none",
        stroke: "#d97706",
        "stroke-width": 2,
      }))
        r.setAttribute(name, String(value));
      svg.append(r);
    }
    wrap.append(svg);
  }
  figure.append(wrap);
  return figure;
}

export function renderDetail(record, data, overlay = false) {
  const box = el("div", "space-y-5");
  const head = el("div");
  head.append(
    el(
      "p",
      "mb-2 font-mono text-xs text-teal-700",
      `${record.id} · ${timeLabel(record.ms)}`,
    ),
    el("h2", "text-xl font-semibold tracking-tight", record.title),
  );
  const badges = el("div", "mt-3 flex flex-wrap gap-2");
  badges.append(
    pill(typeLabel[record.type]),
    pill(record.app ?? "应用未知"),
    pill(record.type === "cdp" ? "基础能力 + demo 接线" : "基础服务实测"),
  );
  if (record.origin) badges.append(pill(`输入来源：${record.origin}`));
  head.append(badges);
  head.append(el("p", "mt-3 text-sm leading-6 text-stone-600", record.note));
  box.append(head);
  if (record.type === "cdp") {
    const s = record.raw.selection;
    const card = el("div", "rounded-xl border border-teal-200 bg-teal-50 p-4");
    card.append(
      el("h3", "text-sm font-semibold", "DOM 选区事实"),
      el("p", "mt-2 whitespace-pre-wrap", s?.text || "无选中文字"),
    );
    card.append(
      el(
        "p",
        "mt-2 font-mono text-xs",
        `anchor: node ${s?.anchor?.node ?? "—"} / offset ${s?.anchor?.offset ?? "—"} → focus: node ${s?.focus?.node ?? "—"} / offset ${s?.focus?.offset ?? "—"}`,
      ),
    );
    card.append(
      el(
        "p",
        "mt-2 text-xs text-stone-500",
        "这是文档实例内的节点号与偏移；本次没有经过唯一性验证的 CSS 定位器。",
      ),
    );
    box.append(card);
  }
  const [before, after] = framesFor(record, data);
  if (record.type === "snapshot" || record.type === "clipboard") {
    if (record.type === "snapshot")
      box.append(documentView(after?.focus?.text?.Available));
    const clip = el("div", "rounded-xl border border-stone-200 p-4");
    clip.append(
      el("h3", "text-sm font-semibold", "本次采样的剪贴板状态"),
      el(
        "p",
        "mt-2 whitespace-pre-wrap text-sm",
        after?.clipboard ?? "未取得文本 / 未变化时尚无缓存",
      ),
      el(
        "p",
        "mt-2 font-mono text-xs text-stone-500",
        `sequence=${after?.clipboard_sequence ?? "未知"}；内容状态=${JSON.stringify(after?.clipboard_observation?.content)}`,
      ),
    );
    box.append(clip);
  }
  const frames = el("div", "grid gap-3 xl:grid-cols-2");
  frames.append(
    frameView(before, data, "相邻前帧", overlay),
    frameView(after, data, "相邻后帧 / 当前采样帧", overlay),
  );
  box.append(frames);
  box.append(
    el(
      "p",
      "text-xs leading-5 text-stone-500",
      "帧按同窗口和时间邻近关联，仅用于对照，不证明该输入导致了画面变化。图片已压缩为展示副本，原始 BMP 未改写。",
    ),
  );
  if (after) {
    box.append(jsonBlock(after.ocr, "展开 OCR 文字与置信度"));
    if (record.type !== "snapshot")
      box.append(jsonBlock(after.focus, "相邻 UIA 快照（非输入瞬间）"));
  }
  box.append(
    jsonBlock(record.raw, `查看原始字段 · ${record.file}:${record.line}`),
  );
  box.append(
    sourceButtons(
      record.type === "cdp"
        ? ["observer", "cdp"]
        : record.type === "input"
          ? ["listener"]
          : ["sampler", "structure", "clipboard"],
      data,
    ),
  );
  return box;
}
