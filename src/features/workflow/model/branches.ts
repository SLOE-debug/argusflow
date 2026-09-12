import type { Value, WorkflowFile } from "./contracts";
import { childScopes, emptyScope, newId } from "./factory";
import { endpointId, initialScopeLayout } from "./endpoints";
import { nodeById, scopeById, updateNode } from "./graph";

/** 添加分支必须同时安装子图、起止卡片和显式连接布局。 */
export function addSwitchCase(
  file: WorkflowFile,
  id: string,
  value: Value,
): WorkflowFile {
  const action = nodeById(file, id)?.action;
  if (action?.kind !== "switch") throw new Error("分支节点已不存在");
  const scope = emptyScope(newId("scope"));
  const next = updateNode(file, id, (node) => ({
    ...node,
    action: { ...action, cases: [...action.cases, { value, scope: scope.id }] },
  }));
  return {
    ...next,
    definition: {
      ...next.definition,
      scopes: [...next.definition.scopes, scope],
    },
    editor: {
      ...next.editor,
      nodes: { ...next.editor.nodes, ...initialScopeLayout(scope.id) },
      edges: {
        ...next.editor.edges,
        [scope.edges[0].id]: { source: "right", target: "left" },
      },
    },
  };
}
/** 移除分支的完整所有权闭包，连线布局与起止卡片一并释放。 */
export function removeSwitchCase(
  file: WorkflowFile,
  id: string,
  scopeId: string,
): WorkflowFile {
  const action = nodeById(file, id)?.action;
  if (action?.kind !== "switch") throw new Error("分支节点已不存在");
  const removed = new Set<string>();
  const collect = (scope: string) => {
    if (removed.has(scope)) return;
    removed.add(scope);
    for (const node of scopeById(file, scope).nodes)
      childScopes(node.action).forEach((child) => collect(child.id));
  };
  collect(scopeId);
  const scopes = file.definition.scopes.filter((scope) =>
    removed.has(scope.id),
  );
  const nodes = new Set(
    scopes.flatMap((scope) => [
      endpointId(scope.id, "start"),
      endpointId(scope.id, "end"),
      ...scope.nodes.map((node) => node.id),
    ]),
  );
  const edges = new Set(
    scopes.flatMap((scope) => scope.edges.map((edge) => edge.id)),
  );
  const next = updateNode(file, id, (node) => ({
    ...node,
    action: {
      ...action,
      cases: action.cases.filter((item) => item.scope !== scopeId),
    },
  }));
  return {
    ...next,
    definition: {
      ...next.definition,
      scopes: next.definition.scopes.filter((scope) => !removed.has(scope.id)),
    },
    editor: {
      nodes: Object.fromEntries(
        Object.entries(next.editor.nodes).filter(([key]) => !nodes.has(key)),
      ),
      edges: Object.fromEntries(
        Object.entries(next.editor.edges).filter(([key]) => !edges.has(key)),
      ),
      drafts: Object.fromEntries(
        Object.entries(next.editor.drafts).filter(
          ([key]) =>
            !nodes.has(key.split(":")[0]) && key !== id + ":case." + scopeId,
        ),
      ),
    },
  };
}
