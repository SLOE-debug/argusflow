import type { WorkflowFile } from "./contracts";
import { childScopes } from "./factory";
import { endpointId, endpointNodeId } from "./endpoints";
import { endpointKey, graphIndex } from "./connections";
import { scopeById } from "./graph";

/** 删除只续接单入单出链；分叉节点不猜测应连接哪条后续线路。 */
export function deleteNodes(
  file: WorkflowFile,
  selected: ReadonlySet<string>,
): WorkflowFile {
  if (!selected.size) return file;
  const removed = new Set(selected),
    scopes = new Set<string>();
  const collect = (scopeId: string) => {
    if (scopes.has(scopeId)) return;
    scopes.add(scopeId);
    removed.add(endpointId(scopeId, "start"));
    removed.add(endpointId(scopeId, "end"));
    for (const node of scopeById(file, scopeId).nodes) {
      removed.add(node.id);
      childScopes(node.action).forEach((child) => collect(child.id));
    }
  };
  for (const scope of file.definition.scopes)
    for (const node of scope.nodes)
      if (selected.has(node.id))
        childScopes(node.action).forEach((child) => collect(child.id));
  const edgeLayouts = { ...file.editor.edges };
  const retained = file.definition.scopes
    .filter((scope) => !scopes.has(scope.id))
    .map((scope) => {
      const index = graphIndex(scope);
      let edges = [...scope.edges];
      for (const node of scope.nodes.filter((node) => removed.has(node.id))) {
        const incoming = edges.filter(
          (edge) => edge.target.kind === "node" && edge.target.node === node.id,
        );
        const outgoing = edges.filter(
          (edge) => edge.source.kind === "node" && edge.source.node === node.id,
        );
        edges = edges.filter(
          (edge) => !incoming.includes(edge) && !outgoing.includes(edge),
        );
        if (
          incoming.length === 1 &&
          outgoing.length === 1 &&
          index.incoming({ kind: "node", node: node.id }).length === 1 &&
          index.outgoing({ kind: "node", node: node.id }).length === 1
        ) {
          const from = incoming[0],
            to = outgoing[0];
          if (
            !edges.some(
              (edge) =>
                endpointKey(edge.source) === endpointKey(from.source) &&
                endpointKey(edge.target) === endpointKey(to.target),
            )
          ) {
            edges.push({ ...from, target: to.target });
            edgeLayouts[from.id] = {
              source: edgeLayouts[from.id].source,
              target: edgeLayouts[to.id].target,
            };
          }
        }
      }
      return {
        ...scope,
        nodes: scope.nodes.filter((node) => !removed.has(node.id)),
        edges: edges.filter(
          (edge) =>
            !removed.has(endpointNodeId(scope.id, edge.source)) &&
            !removed.has(endpointNodeId(scope.id, edge.target)),
        ),
      };
    });
  const retainedEdges = new Set(
    retained.flatMap((scope) => scope.edges.map((edge) => edge.id)),
  );
  return {
    ...file,
    definition: { ...file.definition, scopes: retained },
    editor: {
      nodes: Object.fromEntries(
        Object.entries(file.editor.nodes).filter(([id]) => !removed.has(id)),
      ),
      edges: Object.fromEntries(
        Object.entries(edgeLayouts).filter(([id]) => retainedEdges.has(id)),
      ),
      drafts: Object.fromEntries(
        Object.entries(file.editor.drafts).filter(
          ([key]) => !removed.has(key.split(":")[0]),
        ),
      ),
    },
  };
}
