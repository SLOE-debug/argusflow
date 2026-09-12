import type { Action, Scope, WorkflowFile, WorkflowNode } from "./contracts";
import {
  BOOL,
  INT,
  boolean,
  defaultValue,
  integer,
  literal,
  text,
} from "./expressions";
import { taskSpec } from "../nodes/catalog";
import { initialScopeLayout } from "./endpoints";

export function newId(prefix: string): string {
  return prefix + "_" + crypto.randomUUID().replaceAll("-", "").slice(0, 16);
}
export function emptyScope(id: string): Scope {
  return {
    id,
    edges: [
      { id: newId("edge"), source: { kind: "start" }, target: { kind: "end" } },
    ],
    nodes: [],
    outputs: {},
  };
}
/** 新文档不包含执行用伪开始节点。 */
export function createWorkflow(name = "未命名工作流"): WorkflowFile {
  const root = newId("scope");
  const scope = emptyScope(root);
  return {
    id: newId("flow"),
    definition: {
      name,
      inputs: {},
      outputs: {},
      resources: {},
      root,
      scopes: [scope],
      subflows: {},
    },
    editor: {
      nodes: initialScopeLayout(root),
      edges: { [scope.edges[0].id]: { source: "right", target: "left" } },
      drafts: {},
    },
  };
}
/** 创建结构时同时创建独占的子作用域。 */
export function createNode(kind: string): {
  readonly node: WorkflowNode;
  readonly scopes: readonly Scope[];
} {
  const id = newId("node");
  const scopes: Scope[] = [];
  const child = () => {
    const scope = emptyScope(newId("scope"));
    scopes.push(scope);
    return scope.id;
  };
  let action: Action;
  switch (kind) {
    case "let":
      action = {
        kind,
        name: "变量_" + id.slice(-4),
        value_type: INT,
        value: integer("0"),
      };
      break;
    case "assign":
      action = { kind, assignments: [{ name: "", value: integer("0") }] };
      break;
    case "if":
      action = {
        kind,
        condition: boolean(true),
        then_scope: child(),
        else_scope: child(),
      };
      break;
    case "switch":
      action = {
        kind,
        selector: text(""),
        cases: [{ value: { type: "text", value: "选项" }, scope: child() }],
        default_scope: child(),
      };
      break;
    case "for_each":
      action = {
        kind,
        items: {
          kind: "list",
          item_type: INT,
          items: [integer("1"), integer("2"), integer("3")],
        },
        item: "item",
        index: "index",
        body: child(),
        max_iterations: null,
      };
      break;
    case "while":
      action = {
        kind,
        condition: literal(BOOL, defaultValue(BOOL)),
        body: child(),
        max_iterations: 100,
      };
      break;
    case "block":
      action = { kind, scope: child() };
      break;
    case "wait":
      action = { kind, milliseconds: integer("1000") };
      break;
    case "call_workflow":
      action = { kind, workflow: "", inputs: {}, resources: {} };
      break;
    case "break":
    case "continue":
      action = { kind };
      break;
    case "return":
      action = { kind, values: {} };
      break;
    case "fail":
      action = { kind, code: "workflow_failed" };
      break;
    case "release":
      action = { kind, resource: "" };
      break;
    default: {
      const spec = taskSpec(kind);
      if (!spec) throw new Error("节点类型不存在");
      action = {
        kind: "task",
        task: {
          type_id: spec.id,
          version: 1,
          config: Object.fromEntries(
            spec.config
              .filter((field) => !field.optional)
              .map((field) => [field.key, field.initial]),
          ),
          inputs: Object.fromEntries(
            Object.entries(spec.inputs).map(([key, ty]) => [
              key,
              literal(ty, defaultValue(ty)),
            ]),
          ),
          resources: Object.fromEntries(
            Object.keys(spec.resources).map((key) => [key, ""]),
          ),
          resource_outputs: Object.fromEntries(
            Object.keys(spec.creates).map((key) => [
              key,
              key + "_" + id.slice(-4),
            ]),
          ),
          retry: null,
        },
      };
    }
  }
  return {
    node: { id, timeout_ms: null, action, output_bindings: {} },
    scopes,
  };
}
export function childScopes(
  action: Action,
): readonly { readonly id: string; readonly label: string }[] {
  switch (action.kind) {
    case "block":
      return [{ id: action.scope, label: "步骤" }];
    case "if":
      return [
        { id: action.then_scope, label: "条件成立" },
        { id: action.else_scope, label: "否则" },
      ];
    case "switch":
      return [
        ...action.cases.map((item) => ({
          id: item.scope,
          label: String(item.value.value),
        })),
        { id: action.default_scope, label: "默认" },
      ];
    case "while":
    case "for_each":
      return [{ id: action.body, label: "循环体" }];
    default:
      return [];
  }
}
