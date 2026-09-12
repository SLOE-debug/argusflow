import { act, cleanup, fireEvent } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
  buildCanvasScene,
  createWorkflow,
  endpointId,
  scopeEndpoints,
  studio,
  createConnection,
  scopeById,
} from "../../../../src/features/workflow";
import {
  addNode,
  deleteNodes,
  setLayout,
} from "../../../../src/features/workflow/model/graph";
import {
  copyNodes,
  pasteNodes,
} from "../../../../src/features/workflow/model/clipboard";
import {
  canvasEnvironment,
  installCanvas,
  localPoint,
  nestedFixture,
} from "../../support/canvasFixture";
import { nextNode } from "../../support/graph";

beforeEach(() => {
  vi.useFakeTimers();
  canvasEnvironment();
});
afterEach(async () => {
  cleanup();
  await studio.flushAll();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

it("新画板和子流程自带起止及显式直连，重复添加不产生历史", () => {
  const file = createWorkflow();
  installCanvas(file);
  expect(
    buildCanvasScene(file).details[file.definition.root].endpoints,
  ).toHaveLength(2);
  expect(file.definition.scopes[0].edges).toHaveLength(1);
  const before = studio.active!;
  act(() => studio.add("start", { x: 999, y: 999 }));
  act(() => studio.add("end", { x: 999, y: 999 }));
  expect(studio.active!.file).toBe(before.file);
  expect(studio.active!.past).toHaveLength(0);
  act(() => studio.add("block", { x: 100, y: 100 }));
  for (const scope of studio.active!.file.definition.scopes)
    expect(scopeEndpoints(studio.active!.file, scope.id)).toHaveLength(2);
});

it.each(["start", "end"] as const)(
  "%s 可拖动，取消恢复且松手只记录一次，删除后可撤销和重新添加",
  (kind) => {
    const initial = createWorkflow(),
      scope = initial.definition.root,
      id = endpointId(scope, kind);
    const file = setLayout(initial, id, { x: 120, y: 160 });
    const { host } = installCanvas(file);
    const point = localPoint(scope, { x: 190, y: 196 });
    fireEvent.pointerDown(host, { button: 0, ...point });
    expect(studio.active!.selected).toEqual([id]);
    fireEvent.pointerMove(host, {
      clientX: point.clientX + 80,
      clientY: point.clientY + 40,
    });
    fireEvent.keyDown(host, { key: "Escape" });
    expect(studio.active!.file).toBe(file);
    fireEvent.pointerDown(host, { button: 0, ...point });
    fireEvent.pointerMove(host, {
      clientX: point.clientX + 80,
      clientY: point.clientY + 40,
    });
    fireEvent.pointerUp(host, {
      clientX: point.clientX + 80,
      clientY: point.clientY + 40,
    });
    expect(studio.active!.file.editor.nodes[id]).toMatchObject({
      x: 200,
      y: 200,
    });
    expect(studio.active!.past).toHaveLength(1);
    act(() => studio.undo());
    expect(studio.active!.file).toBe(file);
    act(() => studio.select([id]));
    fireEvent.keyDown(host, { key: "Delete" });
    expect(scopeEndpoints(studio.active!.file, scope)).toHaveLength(1);
    expect(studio.active!.file.definition.scopes[0].edges).toHaveLength(0);
    act(() => studio.add(kind, { x: -80, y: 0 }));
    expect(scopeEndpoints(studio.active!.file, scope)).toHaveLength(2);
  },
);

it("子流程起止可命中，拖动和保存维持局部坐标，选择只包含本层", async () => {
  const fixture = nestedFixture(),
    id = endpointId(fixture.scope, "start");
  const file = setLayout(fixture.file, id, { x: 80, y: 260 });
  const { host } = installCanvas(file);
  const point = localPoint(fixture.scope, { x: 150, y: 296 });
  fireEvent.pointerDown(host, { button: 0, ...point });
  fireEvent.pointerMove(host, {
    clientX: point.clientX + 44,
    clientY: point.clientY + 22,
  });
  fireEvent.pointerUp(host, {
    clientX: point.clientX + 44,
    clientY: point.clientY + 22,
  });
  expect(studio.active!.scope).toBe(fixture.scope);
  expect(studio.active!.file.editor.nodes[id]).toMatchObject({
    x: 160,
    y: 300,
  });
  fireEvent.keyDown(host, { key: "a", ctrlKey: true });
  expect(studio.active!.selected).toContain(id);
  expect(studio.active!.selected).not.toContain(
    endpointId(fixture.root, "start"),
  );
  await act(async () => studio.flushAll());
  expect(studio.api.save).toHaveBeenLastCalledWith(studio.active!.file, null);
});

it("起止局部复制不覆盖已有标记；容器复制重建内部边和标记身份", () => {
  const fixture = nestedFixture(),
    id = endpointId(fixture.scope, "start");
  const file = setLayout(fixture.file, id, { x: -80, y: 260 });
  const clipboard = copyNodes(file, fixture.scope, new Set([id]))!;
  expect(
    pasteNodes(file, fixture.scope, clipboard, { x: 999, y: 999 }).file,
  ).toBe(file);
  const without = deleteNodes(
    file,
    new Set([endpointId(fixture.root, "start")]),
  );
  const restored = pasteNodes(without, fixture.root, clipboard, {
    x: -300,
    y: 400,
  });
  expect(
    restored.file.editor.nodes[endpointId(fixture.root, "start")],
  ).toMatchObject({ x: -300, y: 400 });
  const container = copyNodes(
    file,
    fixture.root,
    new Set([fixture.containers[0]]),
  )!;
  const duplicate = pasteNodes(file, fixture.root, container, {
    x: 1200,
    y: 100,
  });
  const scope = duplicate.file.definition.scopes.find(
    (scope) => !file.definition.scopes.some((old) => old.id === scope.id),
  )!;
  expect(scopeEndpoints(duplicate.file, scope.id)[0]).toMatchObject({
    kind: "start",
    x: -80,
    y: 260,
  });
  expect(scope.edges[0].id).not.toBe(
    file.definition.scopes.find((scope) => scope.id === fixture.scope)!.edges[0]
      .id,
  );
  expect(duplicate.file.editor.edges[scope.edges[0].id]).toEqual({
    source: "right",
    target: "left",
  });
  const removed = deleteNodes(file, new Set([fixture.containers[0]]));
  expect(removed.editor.nodes[id]).toBeUndefined();
  expect(
    removed.editor.edges[scopeById(file, fixture.scope).edges[0].id],
  ).toBeUndefined();
});

it("起点显式线可以独立插入，保留其他出口，跨层起点拒绝连接", () => {
  const fixture = nestedFixture(),
    id = endpointId(fixture.scope, "start");
  const connected = createConnection(
    fixture.file,
    fixture.scope,
    id,
    fixture.second,
  );
  const edge = connected.definition.scopes
    .find((scope) => scope.id === fixture.scope)!
    .edges.find(
      (edge) =>
        edge.target.kind === "node" && edge.target.node === fixture.second,
    )!;
  const inserted = addNode(
    connected,
    fixture.scope,
    "let",
    { x: 0, y: 80 },
    { kind: "insert", edge: edge.id },
  );
  expect(
    inserted.file.definition.scopes
      .find((scope) => scope.id === fixture.scope)!
      .edges.find((item) => item.id === edge.id)!.target,
  ).toEqual({ kind: "node", node: inserted.id });
  expect(nextNode(inserted.file, inserted.id, fixture.scope)).toBe(
    fixture.second,
  );
  expect(() =>
    createConnection(fixture.file, fixture.root, id, fixture.outer),
  ).toThrow("端点必须属于当前作用域");
});
