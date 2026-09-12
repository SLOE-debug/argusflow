import type { WorkflowFile, EdgeEndpoint, NodeLayout } from "./contracts";

/** 起止标记属于编辑布局，不新增执行动作；固定键确保每个作用域各有一个。 */
export type EndpointKind = "start" | "end";
export const ENDPOINT_SIZE = { width: 168, height: 72 } as const;
export function isEndpointKind(kind: string): kind is EndpointKind {
  return kind === "start" || kind === "end";
}
export function endpointId(scope: string, kind: EndpointKind): string {
  return `$${kind}:${scope}`;
}
export function endpointKind(scope: string, id: string): EndpointKind | null {
  if (id === endpointId(scope, "start")) return "start";
  if (id === endpointId(scope, "end")) return "end";
  return null;
}
export function scopeEndpoints(file: WorkflowFile, scope: string) {
  return (["start", "end"] as const).flatMap((kind) => {
    const id = endpointId(scope, kind);
    const position = file.editor.nodes[id];
    return position ? [{ id, kind, ...position, ...ENDPOINT_SIZE }] : [];
  });
}

/** 将语义端点映射到可绘制卡片身份。 */
export function endpointNodeId(scope: string, endpoint: EdgeEndpoint): string {
  return endpoint.kind === "node"
    ? endpoint.node
    : endpointId(scope, endpoint.kind);
}
/** 场景身份进入领域边界时转换为明确的端点类型。 */
export function edgeEndpoint(scope: string, id: string): EdgeEndpoint {
  const kind = endpointKind(scope, id);
  return kind ? { kind } : { kind: "node", node: id };
}
/** 每个新作用域都有可见起止，空流程使用显式直连。 */
export function initialScopeLayout(scope: string): Record<string, NodeLayout> {
  return {
    [endpointId(scope, "start")]: { x: -200, y: 104, label: "开始", note: "" },
    [endpointId(scope, "end")]: { x: 400, y: 104, label: "结束", note: "" },
  };
}
