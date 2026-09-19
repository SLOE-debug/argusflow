/** 从录制事实生成正式流程，并把编译错误反馈给同一对话；不读取 actor-private。 */
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { loadEvidence } from "./evidence.mjs";
import { recordingSystemPrompt } from "../model-system.mjs";
import { makeClient } from "./client.mjs";
import { validateContract, saveResult } from "../prompts/result.mjs";
import { verifyFacts } from "./verify-facts.mjs";

const [source, name] = process.argv.slice(2);
if (!source || !/^[a-z0-9-]+$/.test(name ?? ""))
  throw Error("需要录制目录及新结果名称");
const recording = await loadEvidence(resolve(source));
const output = resolve(source, name);
await mkdir(output, { recursive: false });
const facts = Object.fromEntries(
  [...recording.details].map(([id, fact]) => [
    id,
    id.startsWith("sample-")
      ? {
          window: fact.window,
          text: fact.focus?.text,
          clipboard: fact.clipboard,
          clipboard_current: fact.clipboard_current,
        }
      : fact,
  ]),
);
const messages = [
  { role: "system", content: recordingSystemPrompt },
  {
    role: "user",
    content: JSON.stringify({
      instruction:
        "生成完整紧凑 JSON。使用当前任务目录的语义动作，加入前后置校验。此次正文和选区事实已全量提供，无需猜测被省略内容。宿主提供下述借入资源，资源生命周期无需 workflow 创建。记录内的选区/复制可合并为 browser.copy_text，其 query 从实际角色和文本推导，唯一性在回放前探测。发生正文不一致时停止而非重试副作用。所有窗口按实际角色绑定，输出文件路径作为 input。",
      bindings: {
        resources: {
          browser_page: "automation.page",
          browser_window: "automation.window",
          editor_window: "automation.window",
        },
        inputs: { output_path: { type: "text" } },
        initial_state:
          "宿主准备相同页面内容和新建空白 Windows 记事本文档，窗口可能已复用；上述窗口资源身份已核实",
      },
      targets: recording.targets,
      timeline: recording.timeline,
      facts,
    }),
  },
];
const client = await makeClient(output, "qwen3.7-plus-2026-05-26");
const usage = [];
for (let round = 1; round <= 3; round++) {
  const response = await client(messages, [], round, 16000);
  usage.push(response.usage);
  const content = response.choices[0].message.content;
  messages.push({ role: "assistant", content });
  try {
    const result = JSON.parse(content);
    if (!result.workflow)
      throw Error(
        `请逐项核实已提供的任务与宿主绑定，不把缺少录制时的启动事件当作不能借入资源。尚未解决：${JSON.stringify(result.analysis?.unresolved)}`,
      );
    await validateContract(result.workflow);
    verifyFacts(result, recording);
    const ids = new Set(recording.details.keys());
    const references = result.analysis.node_evidence.flatMap(
      (x) => x.evidence_ids,
    );
    if (references.some((id) => !ids.has(id))) throw Error("存在无效证据引用");
    const missed = recording.timeline.filter(
      (e) =>
        ["keyboard_chord", "copy"].includes(e.kind) &&
        !references.includes(e.id),
    );
    if (missed.length)
      throw Error(`必要原始事件缺少节点引用：${JSON.stringify(missed)}`);
    await saveResult(output, result);
    await writeFile(
      resolve(output, "metrics.json"),
      JSON.stringify(
        { model: response.model, rounds: round, usage, executed: false },
        null,
        2,
      ),
    );
    console.log(
      JSON.stringify({
        output,
        nodes: result.workflow.scopes.flatMap((s) => s.nodes).length,
      }),
    );
    break;
  } catch (error) {
    await writeFile(
      resolve(output, `validation-${round}.json`),
      JSON.stringify({ error: String(error) }, null, 2),
    );
    if (round === 3) throw error;
    messages.push({
      role: "user",
      content: `校验失败：${error.message}\n修正后重新输出完整 JSON，不省略节点或证据。`,
    });
  }
}
