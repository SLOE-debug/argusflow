import { describe, expect, it } from "vitest";
import {
  createWorkflow,
  scopeById,
  nodeById,
  updateNode,
  connectNodes,
  loopTemplate,
  buildScene,
} from "../../../src/features/workflow";
import {
  addNode,
  deleteNodes,
} from "../../../src/features/workflow/model/graph";
import {
  copyNodes,
  pasteNodes,
} from "../../../src/features/workflow/model/clipboard";
import { compose, inverse, worldToScreen } from "../../../src/flow";

describe("workflow graph transactions", () => {
  it("inserts after scope entry and in a wire without losing chain order", () => {
    const empty = createWorkflow();
    const first = addNode(empty, empty.definition.root, "wait", { x: 0, y: 0 });
    const second = addNode(first.file, empty.definition.root, "wait", {
      x: 300,
      y: 0,
    });
    const inserted = addNode(
      second.file,
      empty.definition.root,
      "let",
      { x: 150, y: 0 },
      first.id,
    );
    expect(nodeById(inserted.file, first.id)?.next).toBe(inserted.id);
    expect(nodeById(inserted.file, inserted.id)?.next).toBe(second.id);
    const entry = addNode(
      inserted.file,
      empty.definition.root,
      "wait",
      { x: -300, y: 0 },
      "$entry",
    );
    expect(scopeById(entry.file, empty.definition.root).entry).toBe(entry.id);
    expect(nodeById(entry.file, entry.id)?.next).toBe(first.id);
    expect(() =>
      connectNodes(entry.file, empty.definition.root, second.id, first.id),
    ).toThrow();
  });
  it("deletes owned graphs and reconnects adjacent business steps", () => {
    const empty = createWorkflow(),
      root = empty.definition.root;
    const first = addNode(empty, root, "wait", { x: 0, y: 0 });
    const block = addNode(first.file, root, "block", { x: 300, y: 0 });
    const last = addNode(block.file, root, "wait", { x: 600, y: 0 });
    const action = nodeById(last.file, block.id)!.action;
    if (action.kind !== "block") throw new Error("fixture");
    const child = addNode(last.file, action.scope, "wait", { x: 0, y: 0 });
    const deleted = deleteNodes(child.file, new Set([block.id]));
    expect(deleted.definition.scopes).toHaveLength(1);
    expect(nodeById(deleted, first.id)?.next).toBe(last.id);
    expect(deleted.editor.nodes[child.id]).toBeUndefined();
  });
  it("copies a loop across documents preserving internal variables and remapping graph identities", () => {
    const source = loopTemplate();
    const root = scopeById(source, source.definition.root);
    const clipboard = copyNodes(
      source,
      root.id,
      new Set(root.nodes.map((node) => node.id)),
    )!;
    const target = createWorkflow();
    const pasted = pasteNodes(target, target.definition.root, clipboard, {
      x: 50,
      y: 80,
    });
    expect(pasted.file.definition.scopes).toHaveLength(
      source.definition.scopes.length,
    );
    expect(pasted.file.editor.drafts).toEqual({});
    const oldIds = new Set(
      source.definition.scopes.flatMap((scope) => [
        scope.id,
        ...scope.nodes.map((node) => node.id),
      ]),
    );
    for (const scope of pasted.file.definition.scopes)
      for (const node of scope.nodes) {
        expect(oldIds.has(node.id)).toBe(false);
        if (node.next)
          expect(
            scope.nodes.some((candidate) => candidate.id === node.next),
          ).toBe(true);
      }
    const body = pasted.file.definition.scopes.find(
      (scope) => scope.id !== target.definition.root,
    )!;
    expect(body.nodes[0].action.kind).toBe("assign");
  });
  it("marks external input references without binding same-named target inputs", () => {
    let source = createWorkflow(),
      root = source.definition.root;
    const created = addNode(source, root, "wait", { x: 0, y: 0 });
    source = updateNode(created.file, created.id, (node) => ({
      ...node,
      action: { kind: "wait", milliseconds: { kind: "input", name: "delay" } },
    }));
    const clipboard = copyNodes(source, root, new Set([created.id]))!;
    const target = createWorkflow();
    const result = pasteNodes(target, target.definition.root, clipboard, {
      x: 0,
      y: 0,
    });
    expect(
      result.file.editor.drafts[result.selected[0] + ":milliseconds"],
    ).toContain("delay");
  });
  it("duplicates declarations with internal references rewritten as one graph copy", () => {
    const file = loopTemplate(),
      root = scopeById(file, file.definition.root);
    const copy = copyNodes(
      file,
      root.id,
      new Set(root.nodes.map((node) => node.id)),
    )!;
    const pasted = pasteNodes(file, root.id, copy, { x: 500, y: 500 });
    const original = root.nodes.find(
      (node) => node.action.kind === "let",
    )!.action;
    const duplicate = nodeById(pasted.file, pasted.selected[0])!.action;
    if (original.kind !== "let" || duplicate.kind !== "let")
      throw new Error("fixture");
    expect(duplicate.name).not.toBe(original.name);
    expect(pasted.file.editor.drafts).toEqual({});
  });
  it("scope transitions preserve screen position under composed cameras", () => {
    const file = loopTemplate(),
      scene = buildScene(file);
    const child = Object.values(scene.scopes).find((scope) => scope.parent)!;
    const rootCamera = { x: 152, y: -40, zoom: 1.53 };
    const childCamera = compose(rootCamera, child.transform);
    const local = { x: 150, y: 122 };
    const rootPosition = worldToScreen(local, child.transform);
    expect(worldToScreen(local, childCamera)).toEqual(
      worldToScreen(rootPosition, rootCamera),
    );
    const returned = compose(childCamera, inverse(child.transform));
    expect(returned.x).toBeCloseTo(rootCamera.x);
    expect(returned.y).toBeCloseTo(rootCamera.y);
    expect(returned.zoom).toBeCloseTo(rootCamera.zoom);
  });
});
