import { expect } from "vitest";
import {
  endpointNodeId,
  type WorkflowFile,
} from "../../../src/features/workflow";

/** 测试断言必须确认唯一出口，避免将多条边误当成单后继。 */
export function edgeFrom(file: WorkflowFile, scope: string, source: string) {
  const edges = file.definition.scopes
    .find((item) => item.id === scope)!
    .edges.filter((edge) => endpointNodeId(scope, edge.source) === source);
  expect(edges).toHaveLength(1);
  return edges[0];
}
export function nextNode(
  file: WorkflowFile,
  source: string,
  scope = file.definition.root,
): string | null {
  const target = edgeFrom(file, scope, source).target;
  return target.kind === "node" ? target.node : null;
}
