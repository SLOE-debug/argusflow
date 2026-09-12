import { nextNode } from "../../support/graph";
import { act, cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
  buildCanvasScene,
  endpointId,
  nodeById,
  studio,
} from "../../../../src/features/workflow";
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

it("直接点选子节点不改变相机，拖出容器保持归属且只提交一次", () => {
  const fixture = nestedFixture();
  const { host } = installCanvas(fixture.file);
  const before = studio.active!;
  const point = nodePoint(fixture.scope, fixture.first);
  fireEvent.pointerDown(host, { button: 0, ...point });
  expect(studio.active!.scope).toBe(fixture.scope);
  expect(studio.active!.selected).toEqual([fixture.first]);
  expect(studio.active!.viewport).toEqual(before.viewport);
  fireEvent.pointerMove(host, {
    clientX: point.clientX - 600,
    clientY: point.clientY - 120,
  });
  expect(studio.active!.file).toBe(before.file);
  fireEvent.pointerUp(host, {
    clientX: point.clientX - 600,
    clientY: point.clientY - 120,
  });
  expect(studio.active!.past).toHaveLength(before.past.length + 1);
  expect(
    studio
      .active!.file.definition.scopes.find(
        (scope) => scope.id === fixture.scope,
      )!
      .nodes.some((node) => node.id === fixture.first),
  ).toBe(true);
  const after = nodePoint(fixture.scope, fixture.first);
  expect(after.clientX).toBeCloseTo(point.clientX - 600, 5);
  expect(after.clientY).toBeCloseTo(point.clientY - 120, 5);
  act(() => studio.undo());
  expect(studio.active!.file).toEqual(before.file);
});

it("Escape 与指针取消丢弃临时位移，不写入文件或撤销历史", () => {
  const fixture = nestedFixture();
  const { host } = installCanvas(fixture.file);
  const before = studio.active!;
  for (const escape of [true, false]) {
    const point = nodePoint(fixture.scope, fixture.first);
    fireEvent.pointerDown(host, { button: 0, ...point });
    fireEvent.pointerMove(host, {
      clientX: point.clientX + 80,
      clientY: point.clientY + 80,
    });
    if (escape) fireEvent.keyDown(host, { key: "Escape" });
    else fireEvent.pointerCancel(host);
    fireEvent.pointerUp(host, point);
    expect(studio.active!.file).toBe(before.file);
    expect(studio.active!.past).toEqual(before.past);
  }
});

it("跨层 Shift 选择不会混合节点，框选只包含起点所属作用域", () => {
  const fixture = nestedFixture();
  const { host } = installCanvas(fixture.file);
  fireEvent.pointerDown(host, {
    button: 0,
    ...nodePoint(fixture.root, fixture.outer),
  });
  fireEvent.pointerUp(host);
  fireEvent.pointerDown(host, {
    button: 0,
    shiftKey: true,
    ...nodePoint(fixture.scope, fixture.first),
  });
  fireEvent.pointerUp(host);
  expect(studio.active!.selected).toEqual([fixture.first]);
  const scope = buildCanvasScene(fixture.file).scopes[fixture.scope];
  fireEvent.pointerDown(host, {
    button: 0,
    ...localPoint(fixture.scope, {
      x: scope.bounds.x + 5,
      y: scope.bounds.y + 5,
    }),
  });
  fireEvent.pointerMove(
    host,
    localPoint(fixture.scope, {
      x: scope.bounds.x + scope.bounds.width - 5,
      y: scope.bounds.y + scope.bounds.height - 5,
    }),
  );
  fireEvent.pointerUp(host);
  expect(new Set(studio.active!.selected)).toEqual(
    new Set([
      fixture.first,
      fixture.second,
      endpointId(fixture.scope, "start"),
      endpointId(fixture.scope, "end"),
    ]),
  );
});

it("同层端口连线可提交，跨层端口与终止端口不触发误添加", () => {
  const fixture = nestedFixture();
  const { host } = installCanvas(fixture.file);
  fireEvent.pointerDown(host, {
    button: 0,
    ...nodePoint(fixture.scope, fixture.first, "out"),
  });
  fireEvent.pointerUp(host, nodePoint(fixture.root, fixture.outer, "in"));
  expect(studio.active!.file).toBe(fixture.file);
  expect(screen.queryByRole("textbox", { name: "搜索节点" })).toBeNull();
  fireEvent.pointerDown(host, {
    button: 0,
    ...nodePoint(fixture.scope, fixture.first, "out"),
  });
  fireEvent.pointerUp(host, nodePoint(fixture.scope, fixture.second, "in"));
  expect(nextNode(studio.active!.file, fixture.first, fixture.scope)).toBe(
    fixture.second,
  );
  expect(studio.active!.past).toHaveLength(1);
});

it("运行只读时可选中和缩放，禁止拖动、连线与添加", () => {
  const fixture = nestedFixture();
  const { host } = installCanvas(fixture.file);
  vi.spyOn(studio, "readonly", "get").mockReturnValue(true);
  const point = nodePoint(fixture.scope, fixture.first);
  fireEvent.pointerDown(host, { button: 0, ...point });
  fireEvent.pointerMove(host, {
    clientX: point.clientX + 90,
    clientY: point.clientY + 90,
  });
  fireEvent.pointerUp(host);
  expect(studio.active!.selected).toEqual([fixture.first]);
  expect(studio.active!.file).toBe(fixture.file);
  fireEvent.keyDown(host, { key: "Tab" });
  expect(screen.queryByRole("textbox", { name: "搜索节点" })).toBeNull();
  fireEvent.wheel(host, { deltaY: -120, ...point });
  expect(studio.active!.viewport.zoom).toBeGreaterThan(1);
});
