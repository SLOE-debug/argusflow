import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { aqlExamples } from "../support/observation_demo/prompts/aql.mjs";
import {
  validateContract,
  saveResult,
} from "../support/observation_demo/prompts/result.mjs";

/** 通过宿主借入的窗口执行一个有类型、有资源绑定的按键节点。 */
function workflow() {
  return {
    name: "契约样例",
    inputs: {},
    outputs: {},
    resources: { editor: "automation.window" },
    root: "root",
    subflows: {},
    scopes: [
      {
        id: "root",
        outputs: {},
        nodes: [
          {
            id: "save",
            timeout_ms: {
              kind: "literal",
              value_type: { type: "int" },
              value: { type: "int", value: 10000 },
            },
            output_bindings: {},
            action: {
              kind: "task",
              task: {
                type_id: "aql.press_keys",
                version: 1,
                config: {
                  platform: "uia",
                  query: "文档(聚焦=是)",
                  keys: "Control+S",
                },
                inputs: {},
                resources: { scope: "editor" },
                resource_outputs: {},
                retry: null,
              },
            },
          },
        ],
        edges: [
          {
            id: "begin",
            source: { kind: "start" },
            target: { kind: "node", node: "save" },
          },
          {
            id: "finish",
            source: { kind: "node", node: "save" },
            target: { kind: "end" },
          },
        ],
      },
    ],
  };
}
test("提示词的全部 AQL 示例由正式中文解析器验证", async () => {
  assert.deepEqual(await validateContract(null, aqlExamples), {
    contract_valid: true,
    executed: false,
  });
});
test("正式 workflow 可编译；旧格式、虚构节点、非法语法和平台组合被拒绝", async () => {
  assert.equal((await validateContract(workflow())).contract_valid, true);
  await assert.rejects(validateContract({ steps: [], replay_ready: false }));
  let value = workflow();
  value.scopes[0].nodes[0].action.task.type_id = "browser.select_text";
  await assert.rejects(validateContract(value));
  value = workflow();
  value.scopes[0].nodes[0].action.task.config.query = 'css("textarea")';
  await assert.rejects(validateContract(value));
  value = workflow();
  value.scopes[0].nodes[0].action.task.config.platform = "cdp";
  await assert.rejects(validateContract(value));
  value = workflow();
  value.scopes[0].nodes[0].action.task.resources.scope = "missing";
  await assert.rejects(validateContract(value));
});
test("审计与引擎文档分开保存，未解决操作不冒充完整 workflow", async () => {
  const directory = await mkdtemp(join(tmpdir(), "argusflow-prompt-"));
  const analysis = {
    summary: "保存请求",
    node_evidence: [
      {
        node_id: "save",
        evidence_ids: ["key-1"],
        outcome: "request_only",
        uncertainty: "保存结果待确认",
      },
    ],
    unresolved: [],
    required_bindings: [
      { name: "editor", kind: "resource", reason: "宿主绑定真实窗口" },
    ],
    replay_ready: false,
  };
  await saveResult(directory, { workflow: workflow(), analysis });
  const saved = JSON.parse(
    await readFile(join(directory, "workflow.json"), "utf8"),
  );
  assert.equal(saved.analysis, undefined);
  assert.equal(saved.scopes[0].nodes[0].action.kind, "task");
  await assert.rejects(saveResult(directory, { workflow: null, analysis }));
  await assert.rejects(
    saveResult(directory, {
      workflow: workflow(),
      analysis: {
        ...analysis,
        unresolved: [{ evidence_ids: ["key-2"], reason: "定位缺失" }],
      },
    }),
  );
  await saveResult(
    directory,
    {
      workflow: null,
      analysis: {
        ...analysis,
        unresolved: [
          { evidence_ids: ["cdp-1"], reason: "必要操作尚无注册任务" },
        ],
      },
    },
    "blocked-",
  );
  await assert.rejects(readFile(join(directory, "blocked-workflow.json")));
});
