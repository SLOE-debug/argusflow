import type { WorkflowFile } from "../model/contracts";
import { buildScene } from "../model/layout";
import { scopeById, setLayout } from "../model/graph";
import {
  edgeEndpoint,
  endpointNodeId,
  scopeEndpoints,
} from "../model/endpoints";
import { graphIndex } from "../model/connections";

/** 按最长拓扑距离分列；同列分支使用稳定行顺序，容器尺寸参与间距。 */
export function tidyScope(file: WorkflowFile, scopeId: string): WorkflowFile {
  const scope = scopeById(file, scopeId);
  const elements = [
    ...scopeEndpoints(file, scopeId),
    ...buildScene(file).scopes[scopeId].nodes,
  ];
  const degrees = new Map(elements.map((node) => [node.id, 0]));
  const columns = new Map(elements.map((node) => [node.id, 0]));
  const index = graphIndex(scope);
  for (const node of elements) {
    degrees.set(
      node.id,
      index
        .incoming(edgeEndpoint(scopeId, node.id))
        .filter((edge) => degrees.has(endpointNodeId(scopeId, edge.source)))
        .length,
    );
  }
  const pending = elements
    .filter((node) => degrees.get(node.id) === 0)
    .map((node) => node.id);
  for (let cursor = 0; cursor < pending.length; cursor++) {
    const source = pending[cursor];
    for (const edge of index.outgoing(edgeEndpoint(scopeId, source))) {
      const target = endpointNodeId(scopeId, edge.target);
      if (!degrees.has(target)) continue;
      columns.set(
        target,
        Math.max(columns.get(target)!, columns.get(source)! + 1),
      );
      degrees.set(target, degrees.get(target)! - 1);
      if (!degrees.get(target)) pending.push(target);
    }
  }
  let result = file,
    x = 80;
  for (let column = 0; column <= Math.max(0, ...columns.values()); column++) {
    const rows = elements.filter((node) => columns.get(node.id) === column);
    let y = 100;
    for (const node of rows) {
      result = setLayout(result, node.id, { x, y });
      y += node.height + 48;
    }
    x += Math.max(168, ...rows.map((node) => node.width)) + 80;
  }
  return result;
}
