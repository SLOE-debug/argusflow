import { act, cleanup, fireEvent } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { studio } from "../../../../src/features/workflow";
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

it.each([0.7, 1, 4])(
  "缩放 %s 时按住 Alt 连续跟随亚像素位移，预览与提交位置一致",
  (zoom) => {
    const { calls } = canvasEnvironment();
    const fixture = nestedFixture();
    const { host } = installCanvas(fixture.file);
    act(() => studio.view({ x: 40, y: 30, zoom }));
    const point = nodePoint(fixture.root, fixture.outer);
    fireEvent.pointerDown(host, { button: 0, ...point });
    act(() => vi.advanceTimersByTime(32));
    const changed = vi.fn();
    const unsubscribe = studio.store.subscribe(changed);
    const bounds = vi.spyOn(host, "getBoundingClientRect");
    for (const distance of [0.5, 1, 1.5, 2.5]) {
      calls.roundRect.mockClear();
      fireEvent.pointerMove(host, {
        altKey: true,
        clientX: point.clientX + distance,
        clientY: point.clientY + distance,
      });
      act(() => vi.advanceTimersByTime(16));
      expect(calls.roundRect).toHaveBeenCalledWith(
        80 + distance / zoom,
        100 + distance / zoom,
        232,
        80,
        12,
      );
    }
    expect(changed).not.toHaveBeenCalled();
    expect(bounds).not.toHaveBeenCalled();
    fireEvent.pointerUp(host, {
      altKey: true,
      clientX: point.clientX + 2.5,
      clientY: point.clientY + 2.5,
    });
    expect(studio.active!.file.editor.nodes[fixture.outer]).toMatchObject({
      x: 80 + 2.5 / zoom,
      y: 100 + 2.5 / zoom,
    });
    expect(studio.active!.past).toHaveLength(1);
    act(() => studio.undo());
    expect(studio.active!.file).toBe(fixture.file);
    unsubscribe();
  },
);

it("节点高频移动合并为一帧，取消丢弃未绘制的位移", () => {
  const { calls } = canvasEnvironment();
  const fixture = nestedFixture();
  const { host } = installCanvas(fixture.file);
  const point = nodePoint(fixture.root, fixture.outer);
  fireEvent.pointerDown(host, { button: 0, ...point });
  act(() => vi.advanceTimersByTime(32));
  const draws = calls.clearRect.mock.calls.length;
  for (let distance = 1; distance <= 100; distance++)
    fireEvent.pointerMove(host, {
      clientX: point.clientX + distance,
      clientY: point.clientY,
    });
  expect(calls.clearRect).toHaveBeenCalledTimes(draws);
  act(() => vi.advanceTimersByTime(16));
  expect(calls.clearRect).toHaveBeenCalledTimes(draws + 1);
  expect(calls.roundRect).toHaveBeenCalledWith(180, 100, 232, 80, 12);
  fireEvent.pointerMove(host, {
    clientX: point.clientX + 150,
    clientY: point.clientY,
  });
  fireEvent.pointerCancel(host);
  calls.roundRect.mockClear();
  act(() => vi.advanceTimersByTime(16));
  expect(calls.roundRect).toHaveBeenCalledWith(80, 100, 232, 80, 12);
  expect(calls.roundRect).not.toHaveBeenCalledWith(230, 100, 232, 80, 12);
  expect(studio.active!.file).toBe(fixture.file);
  expect(studio.active!.past).toHaveLength(0);
});
