import type { WorkflowFile } from "./contracts";
import { scopeById } from "./graph";
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
  let id = scope.entry;
  const visited = new Set<string>();
  while (id && !visited.has(id)) {
    visited.add(id);
    const node = scope.nodes.find((item) => item.id === id);
    if (!node) return false;
    const action = node.action;
    if (["return", "fail", "break", "continue"].includes(action.kind))
      return false;
    if (action.kind === "block" && !child(action.scope)) return false;
    if (
      action.kind === "if" &&
      !child(action.then_scope) &&
      !child(action.else_scope)
    )
      return false;
    if (
      action.kind === "switch" &&
      !child(action.default_scope) &&
      action.cases.every((item) => !child(item.scope))
    )
      return false;
    id = node.next;
  }
  return id === null;
}
