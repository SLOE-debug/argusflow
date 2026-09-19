/** 对同一录制执行精简输入单轮或多轮对照实验。 */
import { resolve } from "node:path";
import { loadEvidence } from "./evidence.mjs";
import { runDialogue } from "./dialogue.mjs";
const [directory, mode, name, model = "qwen3.7-plus-2026-05-26"] =
  process.argv.slice(2);
if (
  !directory ||
  !["single", "multi"].includes(mode) ||
  !/^[a-z0-9-]+$/.test(name ?? "")
)
  throw Error("用法：main.mjs <录制目录> <single|multi> <新输出名称>");
const recording = await loadEvidence(resolve(directory));
if (
  ![
    "qwen3-vl-flash-2026-01-22",
    "qwen3.7-plus-2026-05-26",
    "qwen3.8-max-0902",
  ].includes(model)
)
  throw Error("模型不在本次评估白名单");
console.log(JSON.stringify({ entries: recording.timeline.length, mode }));
const result = await runDialogue(
  recording,
  resolve(directory, name),
  mode,
  model,
);
console.log(
  JSON.stringify({
    nodes:
      result.workflow?.scopes.reduce((n, scope) => n + scope.nodes.length, 0) ??
      0,
    unresolved: result.analysis.unresolved.length,
    ...result.metrics,
    log: undefined,
  }),
);
