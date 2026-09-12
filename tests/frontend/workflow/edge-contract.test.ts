import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
  assertSaveable,
  childScopes,
  createConnection,
  createWorkflow,
  endpointId,
  graphIndex,
  nodeById,
  reconnectEdge,
  removeEdge,
  WorkflowStudio,
} from "../../../src/features/workflow";
import {
  addNode,
  deleteNodes,
} from "../../../src/features/workflow/model/graph";
import {
  copyNodes,
  pasteNodes,
} from "../../../src/features/workflow/model/clipboard";
import { testDesktopApi } from "../support/desktopApi";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());
function draft() {
  const empty = createWorkflow(),
    root = empty.definition.root;
  const a = addNode(empty, root, "wait", { x: 0, y: 0 }, null);
  const b = addNode(a.file, root, "wait", { x: 300, y: 0 }, null);
  const c = addNode(b.file, root, "wait", { x: 600, y: 0 }, null);
  return { file: c.file, root, a: a.id, b: b.id, c: c.id };
}
it("独立连接允许分叉汇合，拒绝重复、自环、回环、跨域与错误方向", () => {
  const f = draft();
  let file = createConnection(f.file, f.root, f.a, f.b);
  file = createConnection(file, f.root, f.a, f.c);
  file = createConnection(file, f.root, f.b, f.c);
  expect(
    graphIndex(file.definition.scopes[0]).outgoing({ kind: "node", node: f.a }),
  ).toHaveLength(2);
  expect(
    graphIndex(file.definition.scopes[0]).incoming({ kind: "node", node: f.c }),
  ).toHaveLength(2);
  expect(
    graphIndex(file.definition.scopes[0]).successor({
      kind: "node",
      node: f.a,
    }),
  ).toBeUndefined();
  for (const [source, target, reason] of [
    [f.a, f.b, "已连接"],
    [f.a, f.a, "自身"],
    [f.c, f.a, "回环"],
    [f.a, "foreign", "作用域"],
    [f.a, endpointId(f.root, "start"), "开始"],
    [endpointId(f.root, "end"), f.a, "结束"],
  ])
    expect(() => createConnection(file, f.root, source, target)).toThrow(
      reason,
    );
  const terminal = addNode(file, f.root, "return", { x: 900, y: 0 }, null);
  expect(() =>
    createConnection(terminal.file, f.root, terminal.id, f.a),
  ).toThrow("终止");
  expect(() => assertSaveable(file)).not.toThrow();
});
it("插线按 ID 拆分，单入单出删除续接，分叉删除仅移除关联边", () => {
  const f = draft();
  let file = createConnection(f.file, f.root, f.a, f.b, {
    source: "top",
    target: "bottom",
  });
  const edge = file.definition.scopes[0].edges.at(-1)!;
  const inserted = addNode(
    file,
    f.root,
    "wait",
    { x: 150, y: 100 },
    { kind: "insert", edge: edge.id },
  );
  const restored = deleteNodes(inserted.file, new Set([inserted.id]));
  expect(restored.definition.scopes[0].edges).toContainEqual(edge);
  expect(restored.editor.edges[edge.id]).toEqual({
    source: "top",
    target: "bottom",
  });
  file = createConnection(restored, f.root, f.a, f.c);
  const deleted = deleteNodes(file, new Set([f.a]));
  expect(deleted.definition.scopes[0].edges).toHaveLength(1);
  expect(Object.keys(deleted.editor.edges)).toHaveLength(1);
});
it("局部剪贴板重建边身份并保留边位，不要求起止卡片", () => {
  const f = draft(),
    file = createConnection(f.file, f.root, f.a, f.b, {
      source: "bottom",
      target: "top",
    });
  const clip = copyNodes(file, f.root, new Set([f.a, f.b]))!;
  expect(clip.format).toBe("argusflow.nodes");
  expect(() => assertSaveable(clip.file)).toThrow("缺少");
  const pasted = pasteNodes(file, f.root, clip, { x: 0, y: 500 });
  const edge = pasted.file.definition.scopes[0].edges.at(-1)!;
  expect(edge.id).not.toBe(clip.file.definition.scopes[0].edges[0].id);
  expect(edge.source).toEqual({ kind: "node", node: pasted.selected[0] });
  expect(edge.target).toEqual({ kind: "node", node: pasted.selected[1] });
  expect(pasted.file.editor.edges[edge.id]).toEqual({
    source: "bottom",
    target: "top",
  });
  const moved = reconnectEdge(
    pasted.file,
    f.root,
    edge.id,
    "target",
    f.c,
    "left",
  );
  expect(moved.definition.scopes[0].edges.at(-1)!.id).toBe(edge.id);
});
it.each(["root", "child"])(
  "%s 起止缺失保留草稿、问题可定位且不覆盖磁盘；补齐后可保存",
  async (location) => {
    const api = testDesktopApi(),
      studio = new WorkflowStudio(api);
    await studio.initializeWorkspace();
    studio.create();
    const added = addNode(studio.active!.file, studio.active!.scope, "while", {
      x: 0,
      y: 0,
    });
    studio.edit(() => added.file);
    await studio.flushAll();
    vi.mocked(api.save).mockClear();
    const scope =
      location === "root"
        ? added.file.definition.root
        : childScopes(nodeById(added.file, added.id)!.action)[0].id;
    studio.edit((file) =>
      deleteNodes(file, new Set([endpointId(scope, "end")])),
    );
    const draft = studio.active!.file;
    await expect(studio.flushAll()).rejects.toThrow("缺少结束");
    expect(api.save).not.toHaveBeenCalled();
    expect(studio.active!.file).toBe(draft);
    expect(studio.active!.status).toBe("failed");
    expect(studio.store.getState().problems).toContainEqual(
      expect.objectContaining({
        workflow: draft.id,
        scope,
        code: "missing_endpoint",
      }),
    );
    studio.edit(
      (file) => addNode(file, scope, "end", { x: 1000, y: 100 }).file,
    );
    await studio.flushAll();
    expect(api.save).toHaveBeenCalledOnce();
    expect(studio.active!.status).toBe("saved");
    expect(studio.store.getState().problems).toEqual([]);
  },
);
it("未连完与多出口草稿往返保存保留全部边", async () => {
  const f = draft(),
    file = createConnection(
      createConnection(f.file, f.root, f.a, f.b),
      f.root,
      f.a,
      f.c,
    );
  const api = testDesktopApi(),
    studio = new WorkflowStudio(api);
  await studio.initializeWorkspace();
  studio.create(file);
  await studio.flushAll();
  const saved = vi.mocked(api.save).mock.calls[0][0];
  expect(saved).toEqual(file);
  const disconnected = removeEdge(
    file,
    f.root,
    file.definition.scopes[0].edges[0].id,
  );
  expect(() => assertSaveable(disconnected)).not.toThrow();
});
