import { act, cleanup, fireEvent } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
  buildCanvasScene,
  createWorkflow,
  endpointId,
  setLayout,
  studio,
} from "../../../../src/features/workflow";
import { addNode } from "../../../../src/features/workflow/model/graph";
import { NodeDragGesture } from "../../../../src/components/workflow/canvas/nodeDragGesture";
import * as guides from "../../../../src/components/workflow/canvas/rendering/guides";
import { compose } from "../../../../src/flow";
import {
  canvasEnvironment,
  installCanvas,
  nestedFixture,
  nodePoint,
} from "../../support/canvasFixture";

beforeEach(() => vi.useFakeTimers());
afterEach(async () => {
  cleanup();
  await studio.flushAll();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

function fixture() {
  let file = createWorkflow();
  const root = file.definition.root;
  file = setLayout(file, endpointId(root, "start"), { x: -500, y: -500 });
  file = setLayout(file, endpointId(root, "end"), { x: 1300, y: 900 });
  const node = addNode(file, root, "wait", { x: 100, y: 120 }, null);
  const vertical = addNode(node.file, root, "wait", { x: 420, y: 500 }, null);
  const horizontal = addNode(
    vertical.file,
    root,
    "wait",
    { x: 800, y: 300 },
    null,
  );
  return { file: horizontal.file, root, node: node.id };
}

it("同时显示横纵参考线，连续更新合并为一帧，松手一次提交并可撤销重做", async () => {
  const { calls } = canvasEnvironment();
  const draw = vi.spyOn(guides, "drawSnapGuides");
  const data = fixture();
  const { host } = installCanvas(data.file);
  await act(async () => studio.flushAll());
  vi.mocked(studio.api.save).mockClear();
  const point = nodePoint(data.root, data.node);
  fireEvent.pointerDown(host, { button: 0, ...point });
  act(() => vi.advanceTimersByTime(32));
  draw.mockClear();
  const changed = vi.fn();
  const unsubscribe = studio.store.subscribe(changed);
  const draws = calls.clearRect.mock.calls.length;
  for (let i = 0; i < 100; i++)
    fireEvent.pointerMove(host, {
      clientX: point.clientX + 316 + i / 100,
      clientY: point.clientY + 176,
    });
  expect(draw).not.toHaveBeenCalled();
  act(() => vi.advanceTimersByTime(16));
  expect(calls.clearRect).toHaveBeenCalledTimes(draws + 1);
  expect(draw).toHaveBeenCalledOnce();
  expect(draw.mock.calls[0][1]).toEqual([
    { axis: "x", position: 536, start: 300, end: 580 },
    { axis: "y", position: 340, start: 420, end: 1032 },
  ]);
  expect(changed).not.toHaveBeenCalled();
  expect(studio.active!.file).toBe(data.file);
  act(() => vi.advanceTimersByTime(1000));
  expect(studio.api.save).not.toHaveBeenCalled();
  fireEvent.pointerUp(host, {
    clientX: point.clientX + 317,
    clientY: point.clientY + 176,
  });
  expect(studio.active!.file.editor.nodes[data.node]).toMatchObject({
    x: 420,
    y: 300,
  });
  expect(studio.active!.past).toHaveLength(1);
  draw.mockClear();
  act(() => vi.advanceTimersByTime(16));
  expect(draw).not.toHaveBeenCalled();
  act(() => studio.undo());
  expect(studio.active!.file).toBe(data.file);
  act(() => studio.redo());
  expect(studio.active!.file.editor.nodes[data.node]).toMatchObject({
    x: 420,
    y: 300,
  });
  unsubscribe();
});

it.each([0.7, 1, 4])(
  "缩放 %s 时只需移出 5 屏幕像素即可脱离，松手采样最终位置",
  (zoom) => {
    const { calls } = canvasEnvironment();
    const data = fixture();
    const { host } = installCanvas(data.file);
    act(() => studio.view({ x: 40 - 160 * zoom, y: 30 - 50 * zoom, zoom }));
    const point = nodePoint(data.root, data.node);
    fireEvent.pointerDown(host, { button: 0, ...point });
    for (const pixels of [-4, 4, 6, -4]) {
      calls.roundRect.mockClear();
      fireEvent.pointerMove(host, {
        clientX: point.clientX + 320 * zoom + pixels,
        clientY: point.clientY + 90 * zoom,
      });
      act(() => vi.advanceTimersByTime(16));
      const drawn = calls.roundRect.mock.calls.find(
        ([, y, width]) => y === 210 && width === 232,
      );
      expect(drawn?.[0]).toBeCloseTo(
        Math.abs(pixels) <= 5 ? 420 : 420 + pixels / zoom,
        8,
      );
    }
    /** 最后的 pointerup 可以在尚未收到 pointermove 时越过吸附范围。 */
    fireEvent.pointerUp(host, {
      clientX: point.clientX + 320 * zoom + 6,
      clientY: point.clientY + 90 * zoom,
    });
    expect(studio.active!.file.editor.nodes[data.node].x).toBeCloseTo(
      420 + 6 / zoom,
    );
  },
);

it("按住 Alt 立即释放吸附，松开恢复；单击不移动节点", () => {
  const { calls } = canvasEnvironment();
  const data = fixture();
  const { host } = installCanvas(data.file);
  const point = nodePoint(data.root, data.node);
  fireEvent.pointerDown(host, { button: 0, ...point });
  fireEvent.pointerUp(host, point);
  expect(studio.active!.file).toBe(data.file);
  fireEvent.pointerDown(host, { button: 0, ...point });
  fireEvent.pointerMove(host, {
    clientX: point.clientX + 316,
    clientY: point.clientY + 176,
  });
  for (const [type, x, y] of [
    ["keydown", 416, 296],
    ["keyup", 420, 300],
  ] as const) {
    calls.roundRect.mockClear();
    fireEvent(host, new KeyboardEvent(type, { key: "Alt", bubbles: true }));
    act(() => vi.advanceTimersByTime(16));
    expect(calls.roundRect).toHaveBeenCalledWith(x, y, 232, 80, 12);
  }
  fireEvent.pointerUp(host, {
    clientX: point.clientX + 316,
    clientY: point.clientY + 176,
    altKey: true,
  });
  expect(studio.active!.file.editor.nodes[data.node]).toMatchObject({
    x: 416,
    y: 296,
  });
});

it.each(["escape", "cancel", "blur", "document"])(
  "%s 清理吸附和辅助线，不提交布局",
  (reason) => {
    canvasEnvironment();
    const draw = vi.spyOn(guides, "drawSnapGuides");
    const data = fixture();
    const { host } = installCanvas(data.file);
    const point = nodePoint(data.root, data.node);
    fireEvent.pointerDown(host, { button: 0, ...point });
    fireEvent.pointerMove(host, {
      clientX: point.clientX + 316,
      clientY: point.clientY + 176,
    });
    if (reason === "escape") fireEvent.keyDown(host, { key: "Escape" });
    if (reason === "cancel") fireEvent.pointerCancel(host);
    if (reason === "blur") fireEvent.blur(window);
    if (reason === "document") act(() => studio.create());
    draw.mockClear();
    act(() => vi.advanceTimersByTime(16));
    expect(draw).not.toHaveBeenCalled();
    expect(studio.store.getState().tabs[data.file.id].file).toBe(data.file);
    expect(studio.store.getState().tabs[data.file.id].past).toHaveLength(0);
  },
);

it("多选整体吸附，保留节点间距", () => {
  canvasEnvironment();
  const data = fixture();
  const second = addNode(
    data.file,
    data.root,
    "wait",
    { x: 150, y: 260 },
    null,
  );
  const { host } = installCanvas(second.file);
  act(() => studio.select([data.node, second.id], data.root));
  const point = nodePoint(data.root, data.node);
  fireEvent.pointerDown(host, { button: 0, ...point });
  fireEvent.pointerMove(host, {
    clientX: point.clientX + 316,
    clientY: point.clientY + 176,
  });
  fireEvent.pointerUp(host, {
    clientX: point.clientX + 316,
    clientY: point.clientY + 176,
  });
  const a = studio.active!.file.editor.nodes[data.node];
  const b = studio.active!.file.editor.nodes[second.id];
  expect(a).toMatchObject({ x: 420, y: 300 });
  expect({ x: b.x - a.x, y: b.y - a.y }).toEqual({ x: 50, y: 140 });
});

it("嵌套作用域只匹配同层节点，阈值包含作用域缩放", () => {
  const data = nestedFixture();
  const scene = buildCanvasScene(data.file);
  const transform = compose(
    { x: 40, y: 30, zoom: 0.7 },
    scene.scopes[data.scope].transform,
  );
  const drag = new NodeDragGesture(
    scene,
    data.file,
    data.scope,
    [data.first],
    { x: 0, y: 0 },
    transform,
  );
  const result = drag.update(
    { x: 360 * transform.zoom - 4, y: 200 * transform.zoom },
    false,
  );
  expect(result.delta.x).toBeCloseTo(360);
  expect(result.guides).toContainEqual({
    axis: "x",
    position: 556,
    start: 80,
    end: 360,
  });
  /** 第二次离开阈值；父层节点不会被加入该作用域的对齐目标。 */
  expect(
    drag.update({ x: 360 * transform.zoom + 6, y: 200 * transform.zoom }, false)
      .guides,
  ).toEqual([]);
  expect(
    drag.update({ x: 340 * transform.zoom, y: 200 * transform.zoom }, false)
      .guides,
  ).toEqual([]);
});

it("起止卡片参与对齐，选区自身不会成为吸附目标", () => {
  const file = createWorkflow();
  const scope = file.definition.root;
  const scene = buildCanvasScene(file);
  const start = endpointId(scope, "start");
  const end = endpointId(scope, "end");
  const view = { x: 0, y: 0, zoom: 1 };
  const one = new NodeDragGesture(
    scene,
    file,
    scope,
    [start],
    { x: 0, y: 0 },
    view,
  );
  expect(one.update({ x: 596, y: 180 }, false).delta).toEqual({
    x: 600,
    y: 180,
  });
  const both = new NodeDragGesture(
    scene,
    file,
    scope,
    [start, end],
    { x: 0, y: 0 },
    view,
  );
  expect(both.update({ x: 1.25, y: 1.75 }, false)).toMatchObject({
    delta: { x: 1.25, y: 1.75 },
    guides: [],
  });
});

it("只读时不能拖动或显示吸附参考线", () => {
  canvasEnvironment();
  const draw = vi.spyOn(guides, "drawSnapGuides");
  const data = fixture();
  const { host } = installCanvas(data.file);
  act(() => studio.store.setState({ busy: true }));
  const point = nodePoint(data.root, data.node);
  fireEvent.pointerDown(host, { button: 0, ...point });
  fireEvent.pointerMove(host, {
    clientX: point.clientX + 316,
    clientY: point.clientY + 176,
  });
  fireEvent.pointerUp(host, {
    clientX: point.clientX + 316,
    clientY: point.clientY + 176,
  });
  act(() => vi.advanceTimersByTime(16));
  expect(studio.active!.file).toBe(data.file);
  expect(draw).not.toHaveBeenCalled();
  act(() => studio.store.setState({ busy: false }));
});
