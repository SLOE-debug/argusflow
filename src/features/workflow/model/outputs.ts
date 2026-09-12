import type { WorkflowFile } from "./contracts";
import { scopeById } from "./graph";
import { graphIndex, endpointKey } from "./connections";
import type { EdgeEndpoint } from "./contracts";
/** 判断正常出口是否可能到达，供属性面板解释返回节点的结果来源。 */
export function scopeCanComplete(
  file: WorkflowFile,
  scopeId: string,
  ancestors = new Set<string>(),
): boolean {
  if (ancestors.has(scopeId)) return false;
  const scope = scopeById(file, scopeId),
    path = new Set([...ancestors, scopeId]);
  const child = (id: string) => scopeCanComplete(file, id, path);
  const index = graphIndex(scope);
  const pending: EdgeEndpoint[] = index
    .outgoing({ kind: "start" })
    .map((edge) => edge.target);
  const visited = new Set<string>();
  while (pending.length) {
    const endpoint = pending.pop()!;
    if (endpoint.kind === "end") return true;
    if (endpoint.kind !== "node" || visited.has(endpointKey(endpoint)))
      continue;
    visited.add(endpointKey(endpoint));
    const node = scope.nodes.find((item) => item.id === endpoint.node);
    if (!node) continue;
    const action = node.action;
    if (["return", "fail", "break", "continue"].includes(action.kind)) continue;
    if (action.kind === "block" && !child(action.scope)) continue;
    if (
      action.kind === "if" &&
      !child(action.then_scope) &&
      !child(action.else_scope)
    )
      continue;
    if (
      action.kind === "switch" &&
      !child(action.default_scope) &&
      action.cases.every((item) => !child(item.scope))
    )
      continue;
    pending.push(...index.outgoing(endpoint).map((edge) => edge.target));
  }
  return false;
}
