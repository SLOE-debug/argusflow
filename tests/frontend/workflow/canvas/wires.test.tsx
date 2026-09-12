import { act, cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
  buildCanvasScene,
  createConnection,
  createWorkflow,
  endpointId,
  setLayout,
  studio,
} from "../../../../src/features/workflow";
import { addNode } from "../../../../src/features/workflow/model/graph";
import { rectAnchor, type FlowAnchorSide } from "../../../../src/flow";
import {
  canvasEnvironment,
  installCanvas,
  localPoint,
  nestedFixture,
  nodePoint,
} from "../../support/canvasFixture";
import { previewRoutes } from "../../../../src/components/workflow/canvas/rendering/previewRoutes";

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
function fixture() {
  let file = createWorkflow();
  const scope = file.definition.root;
  const ids: string[] = [];
  file = setLayout(
    setLayout(file, endpointId(scope, "start"), { x: 80, y: -160 }),
    endpointId(scope, "end"),
    { x: 850, y: 500 },
  );
  for (const point of [
    { x: 80, y: 80 },
    { x: 500, y: 80 },
    { x: 80, y: 320 },
    { x: 500, y: 320 },
  ]) {
    const added = addNode(file, scope, "wait", point, null);
    file = added.file;
    ids.push(added.id);
  }
  return { file, scope, ids };
}
function port(scope: string, id: string, side: FlowAnchorSide) {
  return localPoint(
    scope,
    rectAnchor(
      buildCanvasScene(studio.active!.file).details[scope].rects.get(id)!,
      side,
    ).point,
  );
}
function drag(
  host: HTMLElement,
  start: { clientX: number; clientY: number },
  end: { clientX: number; clientY: number },
) {
  fireEvent.pointerDown(host, { button: 0, ...start });
  fireEvent.pointerMove(host, end);
  fireEvent.pointerUp(host, end);
}
it("四点仅在 hover 节点及操作点区域显示，离开后隐藏", () => {
  const recording = canvasEnvironment(),
    f = fixture();
  const { host } = installCanvas(f.file);
  act(() => vi.advanceTimersByTime(32));
  recording.calls.arc.mockClear();
  fireEvent.pointerMove(host, nodePoint(f.scope, f.ids[0]));
  act(() => vi.advanceTimersByTime(32));
  expect(
    recording.calls.arc.mock.calls.filter((call) => call[2] === 4),
  ).toHaveLength(4);
  recording.calls.arc.mockClear();
  const point = port(f.scope, f.ids[0], "top");
  fireEvent.pointerMove(host, { ...point, clientY: point.clientY - 7 });
  act(() => vi.advanceTimersByTime(32));
  expect(
    recording.calls.arc.mock.calls.filter((call) => call[2] === 4),
  ).toHaveLength(4);
  recording.calls.arc.mockClear();
  fireEvent.pointerLeave(host);
  act(() => vi.advanceTimersByTime(32));
  expect(
    recording.calls.arc.mock.calls.filter((call) => call[2] === 4),
  ).toHaveLength(0);
});
it("多出口与汇合保留已有线，主体吸附择边，精确端口保存指定边", () => {
  const f = fixture(),
    { host } = installCanvas(f.file);
  drag(host, port(f.scope, f.ids[0], "bottom"), nodePoint(f.scope, f.ids[1]));
  let scope = studio.active!.file.definition.scopes[0];
  const first = scope.edges.at(-1)!;
  expect(studio.active!.file.editor.edges[first.id]).toEqual({
    source: "bottom",
    target: "left",
  });
  act(() => studio.select([]));
  drag(host, port(f.scope, f.ids[0], "right"), port(f.scope, f.ids[3], "top"));
  act(() => studio.select([]));
  drag(host, port(f.scope, f.ids[2], "right"), port(f.scope, f.ids[3], "left"));
  scope = studio.active!.file.definition.scopes[0];
  expect(scope.edges).toHaveLength(4);
  expect(scope.edges).toContainEqual(first);
  expect(studio.active!.file.editor.edges[scope.edges[2].id]).toEqual({
    source: "right",
    target: "top",
  });
});
it.each(["source", "target"] as const)(
  "点击线独立选中，%s 端改接实时绘制、保持 ID、松手只有一次提交",
  (end) => {
    const recording = canvasEnvironment(),
      f = fixture();
    const file = createConnection(f.file, f.scope, f.ids[0], f.ids[1]);
    const edge = file.definition.scopes[0].edges.at(-1)!;
    const { host } = installCanvas(file);
    act(() => vi.advanceTimersByTime(32));
    const scene = buildCanvasScene(file),
      route = scene.details[f.scope].edges.find((item) => item.id === edge.id)!;
    const line = route.segments.find(
      (s) =>
        s.kind === "line" &&
        Math.hypot(s.to.x - s.from.x, s.to.y - s.from.y) > 50,
    )!;
    fireEvent.pointerDown(host, {
      button: 0,
      ...localPoint(f.scope, {
        x: (line.from.x + line.to.x) / 2,
        y: (line.from.y + line.to.y) / 2,
      }),
    });
    fireEvent.pointerUp(host);
    expect(studio.active!.selectedEdge).toBe(edge.id);
    expect(studio.active!.selected).toEqual([]);
    const before = studio.active!;
    fireEvent.pointerDown(host, {
      button: 0,
      ...localPoint(
        f.scope,
        route[end === "source" ? "sourceAnchor" : "targetAnchor"].point,
      ),
    });
    act(() => vi.advanceTimersByTime(32));
    const draws = recording.calls.clearRect.mock.calls.length;
    const layoutReads = vi.spyOn(host, "getBoundingClientRect");
    for (let i = 0; i < 100; i++)
      fireEvent.pointerMove(host, localPoint(f.scope, { x: 390, y: 200 + i }));
    expect(studio.active!.file).toBe(file);
    expect(studio.active!.past).toBe(before.past);
    expect(studio.api.save).not.toHaveBeenCalled();
    expect(layoutReads).not.toHaveBeenCalled();
    act(() => vi.advanceTimersByTime(32));
    expect(recording.calls.clearRect).toHaveBeenCalledTimes(draws + 1);
    const target = end === "source" ? f.ids[2] : f.ids[3];
    fireEvent.pointerUp(host, port(f.scope, target, "top"));
    const changed = studio.active!.file.definition.scopes[0].edges.find(
      (item) => item.id === edge.id,
    )!;
    expect(changed[end]).toEqual({ kind: "node", node: target });
    expect(studio.active!.file.editor.edges[edge.id][end]).toBe("top");
    expect(studio.active!.past).toHaveLength(1);
    act(() => studio.undo());
    expect(studio.active!.file).toEqual(file);
    act(() => studio.redo());
    expect(studio.active!.file.definition.scopes[0].edges).toContainEqual(
      changed,
    );
    fireEvent.keyDown(host, { key: "Delete" }); // 重做清除选择，重新选线后 Delete 只删除这条线。
    act(() => studio.selectEdge(edge.id, f.scope));
    fireEvent.keyDown(host, { key: "Delete" });
    expect(studio.active!.file.definition.scopes[0].nodes).toHaveLength(4);
    expect(studio.active!.file.definition.scopes[0].edges).toHaveLength(1);
  },
);
it.each(["blank", "self", "escape", "cancel", "blur", "readonly"])(
  "已有线 %s 取消后恢复连接",
  (mode) => {
    const f = fixture();
    const file = createConnection(f.file, f.scope, f.ids[0], f.ids[1]);
    const edge = file.definition.scopes[0].edges.at(-1)!;
    const { host } = installCanvas(file);
    act(() => studio.selectEdge(edge.id, f.scope));
    fireEvent.pointerDown(host, {
      button: 0,
      ...port(f.scope, f.ids[1], "left"),
    });
    const point =
      mode === "self"
        ? nodePoint(f.scope, f.ids[0])
        : localPoint(f.scope, { x: 400, y: 250 });
    fireEvent.pointerMove(host, point);
    if (mode === "escape") fireEvent.keyDown(host, { key: "Escape" });
    if (mode === "cancel") fireEvent.pointerCancel(host);
    if (mode === "blur") fireEvent.blur(window);
    if (mode === "readonly")
      vi.spyOn(studio, "readonly", "get").mockReturnValue(true);
    fireEvent.pointerUp(host, point);
    expect(studio.active!.file).toBe(file);
    expect(studio.active!.past).toHaveLength(0);
    expect(screen.queryByRole("textbox", { name: "搜索节点" })).toBeNull();
  },
);
it("新线空白松手搜索添加并连接，移动无关障碍也重算线路", () => {
  const f = fixture(),
    { host } = installCanvas(f.file);
  drag(
    host,
    port(f.scope, f.ids[0], "right"),
    localPoint(f.scope, { x: 800, y: 250 }),
  );
  const input = screen.getByRole("textbox", { name: "搜索节点" });
  fireEvent.change(input, { target: { value: "声明变量" } });
  fireEvent.keyDown(input, { key: "Enter" });
  expect(studio.active!.past).toHaveLength(1);
  expect(studio.active!.file.definition.scopes[0].edges).toHaveLength(2);
  const file = createConnection(
      setLayout(f.file, f.ids[1], { x: 800, y: 80 }),
      f.scope,
      f.ids[0],
      f.ids[1],
    ),
    scene = buildCanvasScene(file);
  const edge = scene.details[f.scope].edges.at(-1)!;
  const preview = {
    scope: f.scope,
    ids: [f.ids[2]],
    delta: { x: 320, y: -240 },
    guides: [],
  };
  const routed = previewRoutes(scene, preview).find(
    (item) => item.id === edge.id,
  )!;
  expect(routed.points).not.toEqual(edge.points);
  expect(routed.status).toBe("routed");
  expect(previewRoutes(scene, preview)).toBe(previewRoutes(scene, preview));
});

it("嵌套作用域缩放后可改接指定边，右键端点只删除所选线", () => {
  const f = nestedFixture(2);
  const file = createConnection(f.file, f.scope, f.first, f.second);
  const edge = file.definition.scopes
    .find((scope) => scope.id === f.scope)!
    .edges.at(-1)!;
  const { host } = installCanvas(file);
  act(() => {
    studio.view({ x: 50, y: 30, zoom: 2 });
    studio.selectEdge(edge.id, f.scope);
  });
  drag(host, port(f.scope, f.second, "left"), port(f.scope, f.second, "top"));
  expect(studio.active!.file.editor.edges[edge.id].target).toBe("top");
  expect(studio.active!.past).toHaveLength(1);
  fireEvent.contextMenu(host, port(f.scope, f.second, "top"));
  fireEvent.click(screen.getByRole("menuitem", { name: "删除连线" }));
  expect(
    studio
      .active!.file.definition.scopes.find((scope) => scope.id === f.scope)!
      .edges.some((item) => item.id === edge.id),
  ).toBe(false);
  expect(
    studio.active!.file.definition.scopes.find((scope) => scope.id === f.scope)!
      .nodes,
  ).toHaveLength(2);
});
it("改接预览期间切换文档取消手势，不污染任一文件", () => {
  const f = fixture(),
    file = createConnection(f.file, f.scope, f.ids[0], f.ids[1]);
  const edge = file.definition.scopes[0].edges.at(-1)!;
  const { host } = installCanvas(file);
  act(() => studio.selectEdge(edge.id, f.scope));
  fireEvent.pointerDown(host, {
    button: 0,
    ...port(f.scope, f.ids[1], "left"),
  });
  fireEvent.pointerMove(host, localPoint(f.scope, { x: 400, y: 280 }));
  const next = createWorkflow();
  act(() => studio.create(next));
  fireEvent.pointerUp(host, { clientX: 440, clientY: 310 });
  expect(studio.active!.file).toBe(next);
  expect(studio.active!.past).toHaveLength(0);
  expect(studio.store.getState().tabs[file.id].file).toBe(file);
  expect(studio.store.getState().tabs[file.id].past).toHaveLength(0);
});
