import { el, button, pill, timeLabel, typeLabel } from "./dom.mjs";
import { renderDetail } from "./detail.mjs";
import {
  renderSources,
  renderPresets,
  renderModels,
  renderFiles,
} from "./sections.mjs";
const data = JSON.parse(document.querySelector("#evidence-data").textContent);
let active = "timeline",
  selected =
    data.records.find((r) => r.type === "cdp" && r.raw.kind === "copy")?.id ??
    data.records[0].id;
let showAll = false,
  filter = "all",
  query = "",
  overlay = false;
const pages = [
  ["timeline", "观察时间线"],
  ["sources", "能力来源"],
  ["presets", "写死了哪些内容"],
  ["models", "AI 推断对照"],
  ["files", "原始文件"],
];
document.querySelector("#run-date").textContent =
  new Date(data.session.epoch_ms).toLocaleString("zh-CN", {
    timeZone: "Asia/Shanghai",
    hour12: false,
  }) + " · 北京时间";
for (const [name, count] of [
  ["系统输入记录", data.counts.input],
  ["CDP 页面事件", data.counts.cdp],
  ["UIA / 画面采样", data.counts.samples],
  ["剪贴板文本变化", data.counts.clipboard],
]) {
  const card = el(
    "div",
    "rounded-xl border border-stone-200 bg-white px-5 py-4",
  );
  card.append(
    el("p", "text-xs text-stone-500", name),
    el("p", "mt-2 text-2xl font-semibold tabular-nums", count),
  );
  document.querySelector("#stats").append(card);
}
const jump = (id) => {
  if (!data.records.some((r) => r.id === id)) {
    alert("该编号不在事实时间线中；模型引用可能无效。");
    return;
  }
  selected = id;
  active = "timeline";
  showAll = true;
  filter = "all";
  query = "";
  render();
};
function timeline() {
  const box = el("div");
  const controls = el("div", "mb-4 flex flex-wrap items-center gap-3");
  const search = el(
    "input",
    "min-w-48 flex-1 rounded-lg border border-stone-200 bg-white px-3 py-2 text-sm focus:bg-teal-50 outline-none",
  );
  search.placeholder = "搜索文字、事件编号或键名";
  search.setAttribute("aria-label", "搜索录制证据");
  search.value = query;
  const select = el(
    "select",
    "rounded-lg border border-stone-200 bg-white p-2 text-sm focus:bg-teal-50 outline-none",
  );
  select.setAttribute("aria-label", "筛选证据来源");
  for (const [id, label] of [
    ["all", "全部来源"],
    ...Object.entries(typeLabel),
  ]) {
    const option = el("option", "", label);
    option.value = id;
    select.append(option);
  }
  select.value = filter;
  const label = el("label", "flex items-center gap-2 text-sm text-stone-600"),
    checkbox = el("input", "accent-teal-700");
  checkbox.type = "checkbox";
  checkbox.checked = showAll;
  label.append(
    checkbox,
    document.createTextNode("包括重复采样、修饰键和释放事件"),
  );
  controls.append(search, select, label);
  box.append(controls);
  const split = el(
    "div",
    "grid items-start gap-5 lg:grid-cols-[340px_minmax(0,1fr)]",
  );
  const aside = el(
    "aside",
    "overflow-hidden rounded-2xl border border-stone-200 bg-white lg:sticky lg:top-4",
  );
  const count = el(
      "p",
      "border-b border-stone-100 px-4 py-3 text-xs text-stone-500",
    ),
    list = el("div", "max-h-[65vh] overflow-auto p-2");
  list.setAttribute("aria-label", "录制事件列表");
  aside.append(count, list);
  const detail = el(
    "section",
    "min-w-0 rounded-2xl border border-stone-200 bg-white p-5 md:p-6",
  );
  detail.setAttribute("aria-label", "选中事件详情");
  split.append(aside, detail);
  box.append(split);
  const drawDetail = () => {
    const record = data.records.find((r) => r.id === selected);
    detail.replaceChildren();
    const tools = el("div", "mb-4 flex flex-wrap gap-2");
    tools.append(
      button("上一条", () => move(-1), "bg-stone-100"),
      button("下一条", () => move(1), "bg-stone-100"),
      button(
        overlay ? "关闭 OCR 框" : "显示 OCR 框",
        () => {
          overlay = !overlay;
          drawDetail();
        },
        "bg-teal-50 text-teal-800",
      ),
    );
    detail.append(tools, renderDetail(record, data, overlay));
  };
  let shown = [];
  const move = (delta) => {
    const index = shown.findIndex((r) => r.id === selected);
    const next = shown[Math.max(0, Math.min(shown.length - 1, index + delta))];
    if (next) {
      selected = next.id;
      drawList();
      drawDetail();
    }
  };
  const drawList = () => {
    shown = data.records.filter(
      (r) =>
        (showAll || r.important) &&
        (filter === "all" || filter === r.type) &&
        (!query ||
          `${r.id} ${r.title} ${JSON.stringify(r.raw)}`
            .toLowerCase()
            .includes(query.toLowerCase())),
    );
    count.textContent = `显示 ${shown.length} / ${data.records.length} 个证据条目 · 默认折叠重复与中间态`;
    list.replaceChildren();
    if (!shown.length)
      list.append(
        el(
          "p",
          "p-5 text-sm text-stone-500",
          "没有匹配的记录。请调整筛选条件。",
        ),
      );
    for (const row of shown) {
      const current = row.id === selected;
      const entry = button(
        "",
        () => {
          selected = row.id;
          drawList();
          drawDetail();
        },
        `mb-1 block w-full border text-left ${current ? "border-teal-200 bg-teal-50 text-teal-950" : "border-transparent"}`,
      );
      entry.dataset.recordId = row.id;
      entry.setAttribute("aria-pressed", String(current));
      entry.append(
        el(
          "p",
          "mb-1 flex justify-between font-mono text-xs text-stone-500",
          `${timeLabel(row.ms)} · ${row.id}`,
        ),
        el("p", "text-sm leading-6", row.title),
        el(
          "p",
          "mt-1 text-xs text-stone-500",
          `${typeLabel[row.type]} · ${row.app ?? ""}`,
        ),
      );
      list.append(entry);
    }
  };
  search.addEventListener("input", () => {
    query = search.value;
    drawList();
  });
  select.addEventListener("change", () => {
    filter = select.value;
    drawList();
  });
  checkbox.addEventListener("change", () => {
    showAll = checkbox.checked;
    drawList();
  });
  drawList();
  drawDetail();
  return box;
}
function render() {
  const nav = document.querySelector("#navigation");
  nav.replaceChildren();
  for (const [id, label] of pages) {
    const b = button(
      label,
      () => {
        active = id;
        render();
      },
      active === id
        ? "bg-teal-900 text-white hover:bg-teal-800 focus:bg-teal-800"
        : "text-stone-600",
    );
    b.setAttribute("aria-current", active === id ? "page" : "false");
    nav.append(b);
  }
  const views = {
    timeline,
    sources: () => renderSources(data),
    presets: () => renderPresets(data),
    models: () => renderModels(data, jump),
    files: () => renderFiles(data),
  };
  document.querySelector("#page").replaceChildren(views[active]());
}
for (const close of document.querySelectorAll("[data-close]"))
  close.addEventListener("click", () => close.closest("dialog").close());
render();
