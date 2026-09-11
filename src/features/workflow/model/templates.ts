import { addNode, updateNode } from "./graph";
import { createWorkflow } from "./factory";
import { integer } from "./expressions";
import type { WorkflowFile } from "./contracts";
/** 无外部副作用的起步模板，可直接运行验证循环与返回值。 */
export function loopTemplate(): WorkflowFile {
  let file = createWorkflow("循环示例");
  const root = file.definition.root;
  const total = addNode(file, root, "let", { x: 80, y: 100 });
  file = total.file;
  file = updateNode(file, total.id, (node) => ({
    ...node,
    action: {
      kind: "let",
      name: "total",
      value_type: { type: "int" },
      value: integer("0"),
    },
  }));
  const loop = addNode(file, root, "for_each", { x: 360, y: 100 });
  file = loop.file;
  const loopNode = file.definition.scopes
    .flatMap((scope) => scope.nodes)
    .find((node) => node.id === loop.id)!;
  if (loopNode.action.kind !== "for_each") throw new Error("循环模板结构错误");
  const assign = addNode(file, loopNode.action.body, "assign", {
    x: 80,
    y: 80,
  });
  file = assign.file;
  file = updateNode(file, assign.id, (node) => ({
    ...node,
    action: {
      kind: "assign",
      assignments: [
        {
          name: "total",
          value: {
            kind: "binary",
            op: "add",
            left: { kind: "variable", name: "total" },
            right: { kind: "variable", name: "item" },
          },
        },
      ],
    },
  }));
  file = {
    ...file,
    definition: {
      ...file.definition,
      outputs: { total: { type: "int" } },
      scopes: file.definition.scopes.map((scope) =>
        scope.id === root
          ? {
              ...scope,
              outputs: { total: { kind: "variable", name: "total" } },
            }
          : scope,
      ),
    },
  };
  return file;
}
/** 浏览器模板显式创建每个资源，程序路径由用户配置。 */
export function browserTemplate(): WorkflowFile {
  let file = createWorkflow("浏览器操作");
  const root = file.definition.root;
  for (const [index, kind] of [
    "browser.launch",
    "browser.new_page",
    "source.dom",
    "aql.click",
  ].entries()) {
    const result = addNode(file, root, kind, { x: 80 + index * 270, y: 100 });
    file = result.file;
    file = updateNode(file, result.id, (node) => {
      if (node.action.kind !== "task") return node;
      const task = node.action.task;
      if (kind === "browser.launch")
        return {
          ...node,
          action: {
            kind: "task",
            task: { ...task, resource_outputs: { browser: "browser" } },
          },
        };
      if (kind === "browser.new_page")
        return {
          ...node,
          action: {
            kind: "task",
            task: {
              ...task,
              inputs: {
                url: {
                  kind: "literal",
                  value_type: { type: "text" },
                  value: { type: "text", value: "https://example.com" },
                },
              },
              resources: { browser: "browser" },
              resource_outputs: { page: "page" },
            },
          },
        };
      if (kind === "source.dom")
        return {
          ...node,
          action: {
            kind: "task",
            task: {
              ...task,
              resources: { page: "page" },
              resource_outputs: { source: "source" },
            },
          },
        };
      return {
        ...node,
        action: {
          kind: "task",
          task: { ...task, resources: { source: "source" } },
        },
      };
    });
  }
  return file;
}
