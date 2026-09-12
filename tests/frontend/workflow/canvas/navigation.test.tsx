import { act, cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { buildCanvasScene, studio } from "../../../../src/features/workflow";
import {
  compose,
  fitBounds,
  inverse,
  screenToWorld,
  worldToScreen,
} from "../../../../src/flow";
import { locate } from "../../../../src/components/workflow/execution/LogPanel";
import {
  canvasEnvironment,
  installCanvas,
  localPoint,
  nestedFixture,
  nodePoint,
} from "../../support/canvasFixture";

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

it("五层循环可放大到可读尺度，缩放不改变编辑作用域或 Canvas 实例", () => {
  const fixture = nestedFixture(5);
  const { host } = installCanvas(fixture.file);
  const canvas = host.querySelector("canvas");
  const initial = studio.active!.viewport;
  const point = nodePoint(fixture.scope, fixture.first);
  const anchor = screenToWorld({ x: point.clientX, y: point.clientY }, initial);
  fireEvent.wheel(host, { deltaY: -4000, ...point });
  expect(studio.active!.scope).toBe(fixture.root);
  const scene = buildCanvasScene(fixture.file);
  expect(
    studio.active!.viewport.zoom * scene.scopes[fixture.scope].transform.zoom,
  ).toBeCloseTo(4);
  const projected = worldToScreen(anchor, studio.active!.viewport);
  expect(projected.x).toBeCloseTo(point.clientX);
  expect(projected.y).toBeCloseTo(point.clientY);
  fireEvent.pointerDown(host, {
    button: 0,
    ...nodePoint(fixture.scope, fixture.first),
  });
  fireEvent.pointerUp(host);
  expect(studio.active!.selected).toEqual([fixture.first]);
  fireEvent.wheel(host, { deltaY: 500, ...point });
  expect(studio.active!.scope).toBe(fixture.scope);
  expect(host.querySelector("canvas")).toBe(canvas);
  expect(host.querySelectorAll("[data-node], svg")).toHaveLength(0);
});

it("双击与面包屑平滑聚焦同一场景，用户滚轮可中断动画", () => {
  const fixture = nestedFixture();
  const { host } = installCanvas(fixture.file);
  const canvas = host.querySelector("canvas");
  fireEvent.doubleClick(host, nodePoint(fixture.root, fixture.containers[0]));
  expect(studio.active!.scope).toBe(fixture.scope);
  act(() => vi.advanceTimersByTime(220));
  const geometry = buildCanvasScene(fixture.file).scopes[fixture.scope];
  const expected = compose(
    fitBounds(geometry.bounds, 1200, 800),
    inverse(geometry.transform),
  );
  expect(studio.active!.viewport.zoom).toBeCloseTo(expected.zoom);
  fireEvent.click(screen.getByRole("button", { name: "画布测试" }));
  act(() => vi.advanceTimersByTime(32));
  fireEvent.wheel(host, { deltaY: -100, clientX: 400, clientY: 300 });
  const interrupted = studio.active!.viewport;
  act(() => vi.advanceTimersByTime(300));
  expect(studio.active!.viewport).toEqual(interrupted);
  expect(host.querySelector("canvas")).toBe(canvas);
});

it("日志定位采用根相机，居中定位不修改文件且不再显示缩略图", async () => {
  const fixture = nestedFixture();
  const { host } = installCanvas(fixture.file);
  await act(async () =>
    locate({
      workflow: fixture.file.id,
      scope: fixture.scope,
      node: fixture.first,
      instance: "test",
      execution: null,
    }),
  );
  const point = nodePoint(fixture.scope, fixture.first);
  expect(point.clientX).toBeGreaterThan(0);
  expect(point.clientX).toBeLessThan(800);
  expect(studio.active!.scope).toBe(fixture.scope);
  expect(studio.active!.selected).toEqual([fixture.first]);
  const camera = studio.active!.viewport;
  fireEvent.click(screen.getByRole("button", { name: "居中显示" }));
  expect(studio.active!.viewport).not.toEqual(camera);
  expect(screen.queryByRole("button", { name: "小地图" })).toBeNull();
  expect(screen.queryByLabelText("流程概览，点击定位")).toBeNull();
  expect(studio.active!.file).toBe(fixture.file);
  expect(host.querySelectorAll("canvas")).toHaveLength(1);
});

it("中键可平移至负坐标，触控板捏合保持指针锚点", () => {
  const fixture = nestedFixture();
  const { host } = installCanvas(fixture.file);
  fireEvent.pointerDown(host, { button: 1, clientX: 400, clientY: 300 });
  fireEvent.pointerMove(host, { clientX: -5000, clientY: -3000 });
  fireEvent.pointerUp(host, { clientX: -5000, clientY: -3000 });
  expect(studio.active!.viewport.x).toBe(-5360);
  expect(studio.active!.viewport.y).toBe(-3270);
  const point = localPoint(fixture.root, { x: 100, y: 100 });
  const before = studio.active!.viewport;
  const anchor = screenToWorld({ x: point.clientX, y: point.clientY }, before);
  fireEvent.wheel(host, { deltaY: -5, ctrlKey: true, ...point });
  expect(studio.active!.viewport.zoom).toBeGreaterThan(before.zoom);
  expect(worldToScreen(anchor, studio.active!.viewport).x).toBeCloseTo(
    point.clientX,
  );
});
