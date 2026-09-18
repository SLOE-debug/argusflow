import { expect, it } from "vitest";
import {
  createNode,
  createWorkflow,
  integer,
  changeTargetPlatform,
  targetResources,
  browserTemplate,
} from "../../../src/features/workflow";
import {
  addNode,
  updateNode,
} from "../../../src/features/workflow/model/graph";
import { applyQuery } from "../../../src/features/workflow/nodes/query";
import {
  copyNodes,
  pasteNodes,
} from "../../../src/features/workflow/model/clipboard";

it("切换平台保留中文查询与输入，清除旧范围", () => {
  const { node } = createNode("aql.click");
  if (node.action.kind !== "task") throw new Error("task required");
  const task = {
    ...node.action.task,
    config: { platform: "uia", query: "目标(文本=$联系人)" },
    resources: { scope: "微信" },
    inputs: { 联系人: { kind: "input" as const, name: "联系人" } },
  };
  const changed = changeTargetPlatform(task, "cdp");
  expect(changed.resources).toEqual({ scope: "" });
  expect(changed.inputs).toBe(task.inputs);
  expect(changed.config.query).toBe(task.config.query);
  expect(targetResources(changed)).toEqual({ scope: "automation.page" });
  expect(node.timeout_ms).toEqual(integer("10000"));
});

it("应用中文查询保留参数表达式，不将参数值拼接进源码", () => {
  const empty = createWorkflow();
  const added = addNode(empty, empty.definition.root, "aql.exists", {
    x: 0,
    y: 0,
  });
  const query = "目标(文本=$联系人)";
  let file = applyQuery(added.file, added.id, query, {
    联系人: { type: "text" },
  });
  file = updateNode(file, added.id, (node) =>
    node.action.kind === "task"
      ? {
          ...node,
          action: {
            ...node.action,
            task: {
              ...node.action.task,
              inputs: { 联系人: { kind: "input", name: "联系人" } },
            },
          },
        }
      : node,
  );
  file = applyQuery(file, added.id, query, { 联系人: { type: "text" } });
  const node = file.definition.scopes[0].nodes[0];
  if (node.action.kind !== "task") throw new Error("task required");
  expect(node.action.task.config.query).toBe(query);
  expect(node.action.task.inputs.联系人).toEqual({
    kind: "input",
    name: "联系人",
  });
});

it("浏览器模板在动作节点直接绑定页面", () => {
  const nodes = browserTemplate().definition.scopes[0].nodes;
  expect(nodes).toHaveLength(3);
  const action = nodes[2].action;
  if (action.kind !== "task") throw new Error("task required");
  expect(action.task.config.platform).toBe("cdp");
  expect(action.task.resources).toEqual({ scope: "page" });
});

it("跨文档粘贴时限引用会标为待绑定，避免读取同名参数", () => {
  const source = createWorkflow();
  const added = addNode(source, source.definition.root, "wait", { x: 0, y: 0 });
  const file = updateNode(added.file, added.id, (node) => ({
    ...node,
    timeout_ms: { kind: "input", name: "时限" },
  }));
  const clipboard = copyNodes(file, file.definition.root, new Set([added.id]))!;
  const target = createWorkflow();
  const pasted = pasteNodes(target, target.definition.root, clipboard, {
    x: 0,
    y: 0,
  });
  expect(pasted.file.editor.drafts[pasted.selected[0] + ":timeout"]).toContain(
    "时限",
  );
});
