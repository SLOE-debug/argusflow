import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { JSDOM } from "jsdom";
const source = readFileSync(
  new URL(
    "../../../crates/argusflow-browser/src/page/observation.js",
    import.meta.url,
  ),
  "utf8",
);
function fixture() {
  const dom = new JSDOM(
    '<textarea id="editor">相同\n相同\n😀末尾</textarea><input id="secret" type="password" value="private-value"><p>第一段<span>第二段</span></p>',
    { runScripts: "outside-only" },
  );
  dom.window.eval(source);
  return dom;
}
test("原生字段按偏移区分重复文本并保存选区变化事件", () => {
  const dom = fixture(),
    w = dom.window,
    input = w.document.querySelector("textarea");
  input.focus();
  input.setSelectionRange(3, 5, "backward");
  w.document.dispatchEvent(new w.Event("selectionchange"));
  const snapshot = w.__argusflowRecorderV1.take();
  assert.equal(snapshot.active.editable, true);
  assert.deepEqual(JSON.parse(JSON.stringify(snapshot.selection)), {
    kind: "control",
    node: snapshot.active.node,
    start: 3,
    end: 5,
    direction: "backward",
    text: "相同",
    truncated: false,
  });
  assert.equal(snapshot.events.at(-1).selection.start, 3);
  assert.equal(input.selectionStart, 3);
  input.setSelectionRange(6, 8);
  assert.equal(w.__argusflowRecorderV1.take().selection.text, "😀");
  dom.window.close();
});
test("密码选区和值不进入观察结果", () => {
  const dom = fixture(),
    w = dom.window,
    input = w.document.querySelector("input");
  input.focus();
  input.select();
  input.dispatchEvent(new w.Event("copy", { bubbles: true }));
  const snapshot = w.__argusflowRecorderV1.take();
  assert.equal(snapshot.selection.kind, "sensitive");
  assert.equal(snapshot.active.value, null);
  assert.ok(!JSON.stringify(snapshot).includes("private-value"));
  dom.window.close();
});
test("跨节点 DOM 范围保留不同端点，且读取不改选区", () => {
  const dom = fixture(),
    w = dom.window,
    p = w.document.querySelector("p"),
    r = w.document.createRange();
  // jsdom 无布局引擎；只为测试 DOM 范围契约提供矩形接口，不冒充浏览器验收。
  w.Range.prototype.getClientRects = () => [
    { left: 1, top: 2, right: 30, bottom: 20 },
  ];
  r.setStart(p.firstChild, 1);
  r.setEnd(p.lastChild.firstChild, 2);
  w.getSelection().addRange(r);
  const s = w.__argusflowRecorderV1.take().selection;
  assert.equal(s.kind, "document");
  assert.equal(s.text, "一段第二");
  assert.notEqual(s.anchor.node, s.focus.node);
  assert.equal(s.anchor.offset, 1);
  assert.equal(s.focus.offset, 2);
  assert.equal(w.getSelection().toString(), s.text);
  dom.window.close();
});
test("事件队列有上限并显式报告丢失，停止后移除监听", () => {
  const dom = fixture(),
    w = dom.window,
    input = w.document.querySelector("textarea");
  for (let i = 0; i < 200; i++)
    input.dispatchEvent(new w.Event("copy", { bubbles: true }));
  const s = w.__argusflowRecorderV1.take();
  assert.equal(s.events.length, 128);
  assert.equal(s.lost, 72);
  w.__argusflowRecorderV1.stop();
  assert.equal(w.__argusflowRecorderV1, undefined);
  dom.window.close();
});
