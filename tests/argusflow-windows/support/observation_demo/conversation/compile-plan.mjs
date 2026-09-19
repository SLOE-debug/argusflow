/** 模型生成真实任务清单，确定性展开后依次进行引擎与事实校验。 */
import { readFile, writeFile, mkdir } from "node:fs/promises";
import { resolve } from "node:path";
import { evidenceSystemPrompt } from "../model-system.mjs";
import { taskPrompt } from "../prompts/tasks.mjs";
import { aqlPrompt } from "../prompts/aql.mjs";
import { transferPrompt } from "../prompts/transfer.mjs";
import { chatPrompt } from "../prompts/chat.mjs";
import { loadEvidence } from "./evidence.mjs";
import { lowerPlan } from "./lower-plan.mjs";
import { verifyFacts } from "./verify-facts.mjs";
import { validateContract, saveResult } from "../prompts/result.mjs";
import { makeClient } from "./client.mjs";

const [directory, name, feedbackFile, previousFile] = process.argv.slice(2);
if (!directory || !/^[a-z0-9-]+$/.test(name ?? ""))
  throw Error("需要录制目录、新输出名称");
const recording = await loadEvidence(resolve(directory)),
  output = resolve(directory, name);
await mkdir(output, { recursive: false });
const facts = Object.fromEntries(
  [...recording.details].map(([id, f]) => [
    id,
    id.startsWith("sample-")
      ? {
          window: f.window,
          text: f.focus?.text,
          clipboard: f.clipboard,
          clipboard_current: f.clipboard_current,
          documents: f.documents,
          editor_empty: f.editor_empty,
          ocr:
            f.window.title === "微信"
              ? f.ocr.filter((b) => b.rect[0] > f.crop[2] * 0.35)
              : undefined,
        }
      : f,
  ]),
);
const system =
  evidenceSystemPrompt +
  "\n" +
  taskPrompt
    .split("\n")
    .filter(
      (line) =>
        !/^(browser\.(launch|connect|pages|attach|new_page)|application\.|window\.(attach|ocr)|source\.host)/.test(
          line,
        ),
    )
    .join("\n") +
  "\n" +
  aqlPrompt +
  "\n" +
  transferPrompt +
  "\n" +
  chatPrompt +
  `
本接口由确定性编译器展开正式 Workflow。你只返回 {summary:string,nodes:[{id:string,type_id:string,config:object,inputs:object,resources:object,evidence_ids:string[]}]}，紧凑 JSON。
任务名必须来自目录。inputs 传 text/bool/int 原始常量或 {input:"output_path"}、{node:"前序节点ID",output:"输出端口"}；resources 值是已借入资源名字符串。所有任务按执行顺序列出，编译器只展开格式，不补写操作。必须覆盖每个原始 keyboard_chord，包括 Control+End 和末尾的选区/复制；记事本每个按键事件对应单独 aql.press_keys 节点，微信同一轮 Control+V 和 Enter 由上述组合节点覆盖。keys 将 LeftControl/RightControl 规范为 Control，其他同理。每个节点附真实事件ID，保持事实顺序。
已借入 browser_page:automation.page、browser_window:automation.window、editor_window:automation.window；宿主已准备源页面和空白记事本文档、UIA/剪贴板服务。output_path 为新文件运行输入。资源绑定已经由宿主解决。
此紧凑编译入口不声明新资源，不调用 window.attach、window.ocr 或任何资源创建任务。demo.wechat_paste_send 内部自己激活微信，调用它之前不要给微信另写 window.activate，不要绑定历史 HWND 名称。
副作用不重试。每次保存后 file.wait_text；每次原生复制前后 clipboard.checkpoint / clipboard.wait_text；粘贴前定位末尾使用实际观察到的 Control+End。原生复制后预期文字以对应剪贴板采样为准。保护性节点引用所保护事件ID。
文档校验按 LF 表示换行；新建的文件使用 Windows 记事本默认 CRLF，file.wait_text 的 expected 按 CRLF 精确比较。第几个中文查询中的序号从1开始；本次源页面文本角色可以基于实际选中文字匹配，唯一性必须通过运行验证。`;
const base = [
  { role: "system", content: system },
  {
    role: "user",
    content: JSON.stringify({
      targets: recording.targets,
      timeline: recording.timeline,
      facts,
    }),
  },
];
const client = await makeClient(output, "qwen3.7-plus-2026-05-26", 180_000);
let messages = [...base],
  usage = [];
if (feedbackFile && previousFile)
  messages.push(
    { role: "assistant", content: await readFile(previousFile, "utf8") },
    { role: "user", content: await readFile(feedbackFile, "utf8") },
  );
for (let round = 1; round <= 4; round++) {
  const response = await client(messages, [], round, 16000);
  usage.push(response.usage);
  const content = response.choices[0].message.content;
  await writeFile(resolve(output, `plan-${round}.json`), content);
  try {
    const plan = JSON.parse(content),
      result = lowerPlan(plan);
    await validateContract(result.workflow);
    const coverage = verifyFacts(result, recording);
    await saveResult(output, result);
    await writeFile(
      resolve(output, "plan.json"),
      JSON.stringify(plan, null, 2),
    );
    await writeFile(
      resolve(output, "coverage.json"),
      JSON.stringify(coverage, null, 2),
    );
    await writeFile(
      resolve(output, "metrics.json"),
      JSON.stringify({ model: response.model, usage, rounds: round }, null, 2),
    );
    console.log(JSON.stringify({ output, ...coverage }));
    break;
  } catch (error) {
    await writeFile(
      resolve(output, `validation-${round}.json`),
      JSON.stringify({ error: String(error) }, null, 2),
    );
    if (round === 4) throw error;
    messages = [
      ...base,
      { role: "assistant", content },
      {
        role: "user",
        content: `校验失败：${error.message}。请修正并输出完整清单。`,
      },
    ];
  }
}
