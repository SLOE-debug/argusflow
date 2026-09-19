/** 所有录制正文通过 textContent 写入，避免将被观察内容执行为 HTML。 */
export function el(tag, className = "", text) {
  const node = document.createElement(tag);
  node.className = className;
  if (text !== undefined) node.textContent = String(text);
  return node;
}
export const button = (text, action, className = "") => {
  const node = el(
    "button",
    `rounded-lg px-3 py-2 text-sm font-medium hover:bg-stone-100 focus:bg-teal-100 outline-none transition-colors ${className}`,
    text,
  );
  node.type = "button";
  node.addEventListener("click", action);
  return node;
};
export const pill = (text) =>
  el(
    "span",
    "inline-flex rounded-md bg-stone-100 px-2 py-1 text-xs text-stone-600",
    text,
  );
export function jsonBlock(value, label = "原始 JSON") {
  const details = el(
    "details",
    "rounded-xl border border-stone-200 bg-stone-50 p-4",
  );
  details.append(
    el("summary", "cursor-pointer text-sm font-medium", label),
    el(
      "pre",
      "mt-3 max-h-96 overflow-auto whitespace-pre-wrap break-all font-mono text-xs leading-6",
      JSON.stringify(value, null, 2),
    ),
  );
  return details;
}
export function sourceButtons(ids, data) {
  const box = el("div", "mt-3 flex flex-wrap gap-2");
  for (const id of ids)
    box.append(
      button(
        "查看代码 · " + id,
        () => {
          const dialog = document.querySelector("#source-dialog");
          dialog.querySelector("h2").textContent = data.code[id].file;
          dialog.querySelector("pre").textContent = data.code[id].source
            .split("\n")
            .map((s, i) => `${String(i + 1).padStart(3)}  ${s}`)
            .join("\n");
          dialog.showModal();
        },
        "bg-white border border-stone-200 text-xs",
      ),
    );
  return box;
}
export const timeLabel = (ms) =>
  `${ms < 0 ? "−" : "+"}${(Math.abs(ms) / 1000).toFixed(3)}s`;
export const typeLabel = {
  input: "系统输入",
  cdp: "CDP 事件",
  snapshot: "UIA / OCR / 画面",
  clipboard: "剪贴板",
};
