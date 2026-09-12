import { RoutingObstacles } from "../../../src/flow";
import { expect, it } from "vitest";
import { routeEdge } from "../../../src/flow/geometry/routing";
import { bindingDeclarations } from "../../../src/features/workflow/model/bindings";
import {
  addNode,
  nodeById,
  updateNode,
} from "../../../src/features/workflow/model/graph";
import { createWorkflow } from "../../../src/features/workflow/model/factory";
import {
  copyNodes,
  pasteNodes,
} from "../../../src/features/workflow/model/clipboard";

it("routes a horizontal connection around an unrelated obstacle", () => {
  const obstacle = { x: 180, y: 10, width: 150, height: 100 };
  const { points } = routeEdge(
    { point: { x: 100, y: 60 }, side: "right" },
    { point: { x: 500, y: 60 }, side: "left" },
    new RoutingObstacles([{ ...obstacle, id: "obstacle" }]),
  );
  expect(points[0]).toEqual({ x: 100, y: 60 });
  expect(points.at(-1)).toEqual({ x: 500, y: 60 });
  expect(
    points.some(
      (point) => point.y < obstacle.y || point.y > obstacle.y + obstacle.height,
    ),
  ).toBe(true);
});
it("preserves a visible external declaration when pasting into its descendant scope", () => {
  const empty = createWorkflow(),
    root = empty.definition.root;
  const declaration = addNode(empty, root, "let", { x: 0, y: 0 });
  const wait = addNode(declaration.file, root, "wait", { x: 300, y: 0 });
  const container = addNode(wait.file, root, "block", { x: 600, y: 0 });
  const variable = nodeById(container.file, declaration.id)!.action;
  const block = nodeById(container.file, container.id)!.action;
  if (variable.kind !== "let" || block.kind !== "block")
    throw new Error("fixture");
  const file = updateNode(container.file, wait.id, (node) => ({
    ...node,
    action: {
      kind: "wait",
      milliseconds: { kind: "variable", name: variable.name },
    },
  }));
  expect(
    bindingDeclarations(file, block.scope)["variable/" + variable.name],
  ).toBe(declaration.id);
  const clipboard = copyNodes(file, root, new Set([wait.id]))!;
  const pasted = pasteNodes(file, block.scope, clipboard, { x: 50, y: 80 });
  expect(pasted.file.editor.drafts).toEqual({});
});
