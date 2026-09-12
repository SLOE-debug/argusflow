import { act, cleanup, fireEvent } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { studio } from "../../../../src/features/workflow";
import {
  canvasEnvironment,
  installCanvas,
  nestedFixture,
} from "../../support/canvasFixture";

beforeEach(() => vi.useFakeTimers());
afterEach(async () => {
  cleanup();
  await studio.flushAll();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

it("空格平移的高频事件只绘制最新帧，不通知工作台或读取布局，松手提交实际落点", () => {
  const { calls } = canvasEnvironment();
  const { host } = installCanvas(nestedFixture().file);
  act(() => vi.advanceTimersByTime(32));
  const before = studio.active!;
  const changed = vi.fn();
  const unsubscribe = studio.store.subscribe(changed);
  const bounds = vi.spyOn(host, "getBoundingClientRect");
  fireEvent.keyDown(host, { code: "Space" });
  fireEvent.pointerDown(host, { button: 0, clientX: 400, clientY: 300 });
  const initialDraws = calls.clearRect.mock.calls.length;
  for (let i = 1; i <= 100; i++)
    fireEvent.pointerMove(host, { clientX: 400 + i, clientY: 300 + i });
  expect(changed).not.toHaveBeenCalled();
  expect(bounds).not.toHaveBeenCalled();
  expect(calls.clearRect).toHaveBeenCalledTimes(initialDraws);
  act(() => vi.advanceTimersByTime(16));
  expect(calls.clearRect).toHaveBeenCalledTimes(initialDraws + 1);
  expect(calls.translate).toHaveBeenCalledWith(140, 130);
  expect(studio.active).toBe(before);

  // 第二帧也直接更新预览；松手位置比最后一次 move 更新。
  fireEvent.pointerMove(host, { clientX: 530, clientY: 430 });
  act(() => vi.advanceTimersByTime(16));
  expect(calls.translate).toHaveBeenCalledWith(170, 160);
  expect(changed).not.toHaveBeenCalled();
  fireEvent.pointerUp(host, { clientX: 545, clientY: 435 });
  expect(changed).toHaveBeenCalledTimes(1);
  expect(studio.active!.viewport).toEqual({ x: 185, y: 165, zoom: 1 });
  expect(studio.active!.file).toBe(before.file);
  expect(studio.active!.past).toBe(before.past);
  act(() => vi.advanceTimersByTime(32));
  expect(calls.translate).toHaveBeenCalledWith(185, 165);
  const settled = calls.clearRect.mock.calls.length;
  act(() => vi.advanceTimersByTime(200));
  expect(calls.clearRect).toHaveBeenCalledTimes(settled);
  unsubscribe();
});

it.each(["escape", "pointercancel", "blur", "lostcapture"])(
  "%s 取消平移后恢复原相机，不提交已排队的预览",
  (reason) => {
    const { calls } = canvasEnvironment();
    const { host } = installCanvas(nestedFixture().file);
    const before = studio.active!;
    fireEvent.pointerDown(host, { button: 1, clientX: 400, clientY: 300 });
    fireEvent.pointerMove(host, { clientX: 520, clientY: 430 });
    act(() => vi.advanceTimersByTime(32));
    expect(calls.translate).toHaveBeenCalledWith(160, 160);
    fireEvent.pointerMove(host, { clientX: 600, clientY: 500 });
    if (reason === "escape") fireEvent.keyDown(host, { key: "Escape" });
    else if (reason === "pointercancel") fireEvent.pointerCancel(host);
    else if (reason === "lostcapture") fireEvent.lostPointerCapture(host);
    else fireEvent.blur(window);
    calls.translate.mockClear();
    act(() => vi.advanceTimersByTime(32));
    expect(calls.translate).toHaveBeenCalledWith(40, 30);
    expect(calls.translate).not.toHaveBeenCalledWith(240, 230);
    fireEvent.pointerUp(host, { clientX: 600, clientY: 500 });
    expect(studio.active).toBe(before);
  },
);

it("外部定位接管相机，旧平移不能覆盖它", () => {
  const { calls } = canvasEnvironment();
  const { host } = installCanvas(nestedFixture().file);
  fireEvent.pointerDown(host, { button: 1, clientX: 400, clientY: 300 });
  fireEvent.pointerMove(host, { clientX: 500, clientY: 400 });
  const target = { x: -200, y: -100, zoom: 2 };
  act(() => studio.view(target));
  act(() => vi.advanceTimersByTime(32));
  expect(calls.translate).toHaveBeenCalledWith(-200, -100);
  fireEvent.pointerMove(host, { clientX: 600, clientY: 500 });
  fireEvent.pointerUp(host, { clientX: 610, clientY: 510 });
  expect(studio.active!.viewport).toBe(target);
});

it("只读时仍可平移，卸载清理未绘制的预览", () => {
  const { calls } = canvasEnvironment();
  const { host, unmount } = installCanvas(nestedFixture().file);
  vi.spyOn(studio, "readonly", "get").mockReturnValue(true);
  fireEvent.pointerDown(host, { button: 1, clientX: 400, clientY: 300 });
  fireEvent.pointerUp(host, { clientX: -400, clientY: -300 });
  expect(studio.active!.viewport).toEqual({ x: -760, y: -570, zoom: 1 });
  act(() => vi.advanceTimersByTime(32));
  fireEvent.pointerDown(host, { button: 1, clientX: 400, clientY: 300 });
  fireEvent.pointerMove(host, { clientX: 500, clientY: 400 });
  const draws = calls.clearRect.mock.calls.length;
  unmount();
  act(() => vi.advanceTimersByTime(32));
  expect(calls.clearRect).toHaveBeenCalledTimes(draws);
  expect(studio.active!.viewport).toEqual({ x: -760, y: -570, zoom: 1 });
});
