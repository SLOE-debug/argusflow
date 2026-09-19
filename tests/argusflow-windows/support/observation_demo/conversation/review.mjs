/** 将模型原始计划、证据引用和真实引擎结果合并为离线审阅页。 */
import { readFile, writeFile, readdir } from "node:fs/promises";
import { resolve, basename } from "node:path";
import { loadEvidence } from "./evidence.mjs";

const [directory, generated, ...runs] = process.argv.slice(2);
if (!directory || !generated)
  throw Error("需要录制目录、模型输出目录及回放目录");
const root = resolve(directory),
  output = resolve(root, generated);
const json = async (path) => JSON.parse(await readFile(path, "utf8"));
const plan = await json(resolve(output, "plan.json"));
const coverage = await json(resolve(output, "coverage.json"));
const recording = await loadEvidence(root);
const reports = await Promise.all(
  runs.map(async (run) => ({
    name: basename(run),
    ...(await json(resolve(root, run, "result.json"))),
    receipts: await Promise.all(
      (await readdir(resolve(root, run)))
        .filter((name) => /^wechat-\d+\.json$/.test(name))
        .map((name) => json(resolve(root, run, name))),
    ),
  })),
);
const escape = (value) =>
  String(value).replace(
    /[&<>"']/g,
    (c) =>
      ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[
        c
      ],
  );
const pretty = (value) => escape(JSON.stringify(value, null, 2));
const actual = new Set([
  "browser.copy_text",
  "aql.press_keys",
  "demo.wechat_paste_send",
]);
const rows = plan.nodes
  .map(
    (node, index) => `<article>
  <div class="line"><span class="number">${index + 1}</span><strong>${escape(node.id)}</strong><span class="badge">${actual.has(node.type_id) ? "录制动作" : "回放保护"}</span></div>
  <p><code>${escape(node.type_id)}</code> ${escape(node.config.keys ?? node.config.query ?? "")}</p>
  <details><summary>参数与资源</summary><pre>${pretty({ config: node.config, inputs: node.inputs, resources: node.resources })}</pre></details>
  <details><summary>原始证据 · ${node.evidence_ids.length} 个引用</summary>${node.evidence_ids.map((id) => `<h4>${escape(id)}</h4><pre>${pretty(recording.details.get(id) ?? recording.timeline.find((e) => e.id === id) ?? { missing: true })}</pre>`).join("")}</details>
</article>`,
  )
  .join("");
const results = reports
  .map(
    (report) =>
      `<article><div class="line"><strong>${escape(report.name)}</strong><span class="badge">${escape(report.status)}</span></div><p>${escape(report.error ?? "引擎执行完成")}</p><details open><summary>真实磁盘内容</summary><pre>${escape(report.disk_text)}</pre></details>${report.receipts.map((r, i) => `<details open><summary>微信第 ${i + 1} 轮 · ${r.process_succeeded ? "本地新生气泡已确认" : "未确认，已停止"}</summary><p>${escape(r.recipient)}：${escape(r.expected)}</p><pre>${escape(r.stdout)}${escape(r.stderr)}</pre></details>`).join("")}<details><summary>UIA、剪贴板与错误详情</summary><pre>${pretty(report)}</pre></details></article>`,
  )
  .join("");
await writeFile(
  resolve(output, "review.html"),
  `<!doctype html><html lang="zh-CN"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>录制事实 → AI workflow → 实际回放</title>
<style>body{font:16px/1.65 system-ui,sans-serif;background:#f4f6fa;color:#1d2940;margin:0}main{max-width:1080px;margin:auto;padding:40px 24px}h1{font-size:30px;line-height:1.3}h2{margin-top:36px}article{background:white;border:1px solid #dce2ec;border-radius:12px;padding:18px 22px;margin:12px 0}.line{display:flex;align-items:center;gap:12px;flex-wrap:wrap}.number{background:#243f92;color:white;border-radius:50%;width:30px;text-align:center}.badge{font-size:13px;background:#eaf0ff;color:#274e9d;border-radius:12px;padding:2px 10px}pre{white-space:pre-wrap;overflow-wrap:anywhere;background:#f5f7fa;padding:14px;font-size:13px;max-height:460px;overflow:auto}summary{cursor:pointer;margin-top:8px;color:#274e9d}code{font-size:14px}.stats{display:flex;gap:12px}.stats article{flex:1}.stats b{font-size:28px;display:block}a{color:#274e9d}</style>
<main><p>ArgusFlow · 录制回放审阅</p><h1>录制事实 → AI workflow → 实际回放</h1><p>${escape(plan.summary)}</p>
<div class="stats"><article><b>${coverage.covered_events}</b>关键事件已核对</article><article><b>${coverage.nodes}</b>正式引擎节点</article><article><b>${reports.filter((r) => r.status === "Completed").length} / ${reports.length}</b>本页回放完成数</article></div>
<p>AI 决定任务、顺序、参数及证据引用；编译器只展开类型和图结构。浏览器拖选按选区语义执行，不复刻鼠标轨迹。宿主提供独立源页面和空白 Windows 记事本。保护节点是新增校验，不冒充用户录制动作。</p>
${plan.nodes.some((n) => n.type_id === "demo.wechat_paste_send") ? `<article><strong>完整流程：浏览器逐条复制 → 记事本逐条保存 → 记事本逐行复制 → 文件传输助手逐条发送</strong><p>本计划包含 ${plan.nodes.filter((n) => n.type_id === "demo.wechat_paste_send").length} 轮发送。demo.wechat_paste_send 是此 demo 已注册的组合能力，复用现有微信操作与新生气泡验证；并非默认桌面应用内置节点。每轮引用独立的粘贴、Enter 和采样证据，不以同文消息存在作为成功依据。</p></article>` : ""}
<p><a href="workflow.json">正式 workflow JSON</a> · <a href="plan.json">AI 原始计划</a> · <a href="model-result.json">模型分析与证据映射</a></p>
<h2>真实回放结果</h2>${results}<h2>完整执行顺序</h2>${rows}</main></html>`,
);
console.log(resolve(output, "review.html"));
