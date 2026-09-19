import { test } from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { JSDOM, VirtualConsole } from "jsdom";

const artifact = new URL(
  "../support/observation_demo/output/model-03/recording-viewer.html",
  import.meta.url,
);
const html = await readFile(artifact, "utf8");
function open() {
  const errors = [];
  // jsdom 26 不支持 Tailwind 4 的 CSS layer；本测试只验证 DOM 交互，不验证布局。
  const console = new VirtualConsole();
  console.on("jsdomError", (error) => {
    if (error.type !== "css parsing") errors.push(error.message);
  });
  const dom = new JSDOM(html, {
    runScripts: "dangerously",
    virtualConsole: console,
  });
  return { dom, document: dom.window.document, errors };
}
function click(document, label) {
  const target = [...document.querySelectorAll("button")].find(
    (node) => node.textContent === label,
  );
  assert.ok(target, `Missing button: ${label}`);
  target.click();
}
test("实际数据、嵌入图片和原始记录完整", () => {
  const { dom, document, errors } = open();
  const data = JSON.parse(document.querySelector("#evidence-data").textContent);
  assert.equal(data.records.length, 213);
  assert.equal(data.counts.input, 106);
  assert.equal(data.counts.cdp, 41);
  assert.equal(Object.keys(data.images).length, 26);
  assert.ok(
    Object.values(data.images).every((value) =>
      value.startsWith("data:image/png;base64,"),
    ),
  );
  assert.equal(document.querySelectorAll("script[src],link[href]").length, 0);
  assert.ok(
    document.querySelector("style").textContent.includes(".bg-teal-50"),
  );
  assert.deepEqual(errors, []);
  dom.window.close();
});
test("边界说明、预设计划、模型引用和真实选区可以查看", () => {
  const { dom, document, errors } = open();
  click(document, "能力来源");
  assert.match(document.querySelector("#page").textContent, /正式录制/);
  click(document, "写死了哪些内容");
  assert.match(
    document.querySelector("#page").textContent,
    /actor-private.json/,
  );
  click(document, "AI 推断对照");
  click(document, "sample-47");
  assert.equal(
    document
      .querySelector('[data-record-id="sample-47"]')
      .getAttribute("aria-pressed"),
    "true",
  );
  assert.match(document.querySelector("mark").textContent, /核对本周采购清单/);
  click(document, "显示 OCR 框");
  assert.ok(document.querySelector("svg rect"));
  click(document, "原始文件");
  assert.match(document.querySelector("#page").textContent, /SHA-256/);
  assert.deepEqual(errors, []);
  dom.window.close();
});
test("全部原始事件可展开并按真实内容搜索", () => {
  const { dom, document, errors } = open();
  const checkbox = document.querySelector("input[type=checkbox]");
  checkbox.click();
  assert.equal(document.querySelectorAll("[data-record-id]").length, 213);
  const input = document.querySelector('input[aria-label="搜索录制证据"]');
  input.value = "归档已完成的测试结果";
  input.dispatchEvent(new dom.window.Event("input"));
  assert.ok(document.querySelectorAll("[data-record-id]").length > 0);
  assert.ok(document.querySelectorAll("[data-record-id]").length < 213);
  input.value = "<img src=x onerror=alert(1)>";
  input.dispatchEvent(new dom.window.Event("input"));
  assert.equal(document.querySelectorAll("[data-record-id]").length, 0);
  assert.equal(document.querySelectorAll("img[onerror]").length, 0);
  assert.deepEqual(errors, []);
  dom.window.close();
});
