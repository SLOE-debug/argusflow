import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { JSDOM } from "jsdom";
const source = readFileSync(
  new URL(
    "../../../crates/argusflow-browser/src/page/aql/text_selection.js",
    import.meta.url,
  ),
  "utf8",
);
function fixture() {
  const dom = new JSDOM("<p>第一<b>第二</b>第三</p>", {
      runScripts: "outside-only",
    }),
    node = dom.window.document.querySelector("p");
  node.scrollIntoView = () => {};
  node.getBoundingClientRect = () => ({ width: 100, height: 20 });
  node.style.visibility = "visible";
  return { dom, node, select: dom.window.eval(`(${source})`) };
}
test("UTF-16 范围可以跨文本节点，必须核对当前正文", () => {
  const { dom, node, select } = fixture();
  assert.equal(select.call(node, "第一第二第三", 1, 5), "一第二第");
  assert.equal(dom.window.getSelection().toString(), "一第二第");
  assert.throws(() => select.call(node, "旧正文", 0, 2), /TEXT_CHANGED/);
  assert.equal(dom.window.getSelection().toString(), "一第二第");
  dom.window.close();
});
test("无效范围和隐藏目标拒绝产生选区", () => {
  const { dom, node, select } = fixture();
  for (const [start, end] of [
    [0, 0],
    [-1, 3],
    [0, 99],
  ])
    assert.throws(
      () => select.call(node, "第一第二第三", start, end),
      /RANGE_INVALID/,
    );
  node.style.display = "none";
  assert.throws(() => select.call(node, "第一第二第三", 0, 2), /TARGET_HIDDEN/);
  dom.window.close();
});
