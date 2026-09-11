import type { FlowRect, ViewportTransform } from "../../../flow";
import { compose } from "../../../flow";
import type { WorkflowFile, WorkflowNode } from "./contracts";
import { childScopes } from "./factory";
import { scopeById } from "./graph";
import { PORT_LABELS, taskSpec } from "../nodes/catalog";
import { expressionLabel } from "./expressions";

export interface NodeGeometry extends FlowRect {
  readonly id: string;
  readonly children: readonly string[];
}
export interface ScopeGeometry {
  readonly id: string;
  readonly parent: string | null;
  readonly owner: string | null;
  readonly label: string;
  readonly transform: ViewportTransform;
  readonly local: ViewportTransform;
  readonly bounds: FlowRect;
  readonly nodes: readonly NodeGeometry[];
}
export interface Scene {
  readonly scopes: Readonly<Record<string, ScopeGeometry>>;
}
/** 固定预览比例使变更子图时节点可读性和缩放阈值保持稳定。 */
const PREVIEW_SCALE = 0.55;
/** 起止卡片与业务节点间保留完整连线空间，边界计算和渲染共用尺寸。 */
export const SCOPE_ENDPOINT_LAYOUT = {
  width: 88,
  height: 36,
  gap: 48,
} as const;
export function buildScene(file: WorkflowFile): Scene {
  const result: Record<string, ScopeGeometry> = {};
  const measure = (
    scopeId: string,
    parent: string | null,
    owner: string | null,
    label: string,
    depth: number,
  ): ScopeGeometry => {
    if (depth > 64) throw new Error("结构层级超过 64 层");
    const scope = scopeById(file, scopeId);
    const nodes = scope.nodes.map((node): NodeGeometry => {
      const position = file.editor.nodes[node.id] ?? { x: 80, y: 100 };
      const children = childScopes(node.action).map((child) =>
        measure(child.id, scopeId, node.id, child.label, depth + 1),
      );
      let width = 196,
        height = 64;
      if (children.length) {
        width = Math.max(
          300,
          ...children.map((child) => child.bounds.width * PREVIEW_SCALE + 48),
        );
        height =
          44 +
          children.reduce(
            (sum, child) =>
              sum + Math.max(100, child.bounds.height * PREVIEW_SCALE + 48),
            0,
          );
        let top = position.y + 44;
        for (const child of children) {
          result[child.id] = {
            ...child,
            local: {
              x: position.x + 24 - child.bounds.x * PREVIEW_SCALE,
              y: top + 24 - child.bounds.y * PREVIEW_SCALE,
              zoom: PREVIEW_SCALE,
            },
          };
          top += Math.max(100, child.bounds.height * PREVIEW_SCALE + 48);
        }
      }
      return {
        id: node.id,
        ...position,
        width,
        height,
        children: children.map((child) => child.id),
      };
    });
    const endpointSpace =
      SCOPE_ENDPOINT_LAYOUT.width + SCOPE_ENDPOINT_LAYOUT.gap;
    const x = Math.min(0, ...nodes.map((node) => node.x - endpointSpace));
    const y = Math.min(0, ...nodes.map((node) => node.y - 40));
    const right = Math.max(
      240,
      ...nodes.map((node) => node.x + node.width + endpointSpace),
    );
    const bottom = Math.max(
      140,
      ...nodes.map((node) => node.y + node.height + 40),
    );
    const geometry: ScopeGeometry = {
      id: scopeId,
      parent,
      owner,
      label,
      transform: { x: 0, y: 0, zoom: 1 },
      local: { x: 0, y: 0, zoom: 1 },
      bounds: { x, y, width: right - x, height: bottom - y },
      nodes,
    };
    result[scopeId] = geometry;
    return geometry;
  };
  measure(file.definition.root, null, null, file.definition.name, 0);
  const resolve = (id: string, parent: ViewportTransform) => {
    const scope = result[id];
    const transform = compose(parent, scope.local);
    result[id] = { ...scope, transform };
    scope.nodes.forEach((node) =>
      node.children.forEach((child) => resolve(child, transform)),
    );
  };
  resolve(file.definition.root, { x: 0, y: 0, zoom: 1 });
  return { scopes: result };
}
export function nodeSummary(node: WorkflowNode): string {
  switch (node.action.kind) {
    case "task":
      return (
        Object.entries(node.action.task.inputs)
          .map(
            ([name, value]) =>
              (PORT_LABELS[name] ?? name) + " " + expressionLabel(value),
          )
          .join(" · ") ||
        taskSpec(node.action.task.type_id)?.description ||
        ""
      );
    case "let":
      return node.action.name;
    case "call_workflow":
      return node.action.workflow || "选择工作流";
    case "for_each":
      return "每一项 → " + node.action.item;
    case "while":
      return "条件满足时重复";
    case "if":
      return "条件成立 / 否则";
    case "wait":
      return node.action.milliseconds.kind === "literal"
        ? String(node.action.milliseconds.value.value) + " ms"
        : "表达式";
    case "fail":
      return node.action.code;
    case "assign":
      return node.action.assignments
        .map((item) => item.name || "选择变量")
        .join("、");
    default:
      return "";
  }
}
