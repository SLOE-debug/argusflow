import { act, renderHook } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { usePlayback } from "../../../src/features/recorder/usePlayback";
import { recorderApi } from "../../../src/features/recorder/api";
import { timeQpc } from "../../../src/features/recorder/playback";
import type { VideoFrame } from "../../../src/features/recorder/video";
import { clickAction } from "./review-fixtures";
import { overview } from "./playback-fixtures";
vi.mock("../../../src/features/recorder/api", () => ({
  recorderApi: { videoTimeline: vi.fn() },
}));
afterEach(() => vi.useRealTimers());
it("从首帧前或暂停缺口开始播放时，直接请求下一个有效片段", async () => {
  vi.mocked(recorderApi.videoTimeline).mockResolvedValue({
    ...overview,
    segments: [
      { start_ms: 500, end_ms: 1500 },
      { start_ms: 3000, end_ms: 10000 },
    ],
  });
  const { result } = renderHook(() => usePlayback("gaps", clickAction));
  await act(async () => {});
  act(() => result.current.toggle());
  expect(result.current.seek?.qpc).toBe(timeQpc(overview, 500));
  act(() => result.current.pause());
  act(() => result.current.request(2000));
  act(() => result.current.toggle());
  expect(result.current.seek?.qpc).toBe(timeQpc(overview, 3000));
  const active = result.current.seek!.serial;
  act(() => result.current.onReadError("旧位置没有帧", active - 1));
  expect(result.current.playing).toBe(true);
  expect(result.current.error).toBe("");
  act(() => result.current.onReadError("视频读取超时", active));
  expect(result.current.playing).toBe(false);
  expect(result.current.error).toBe("视频读取超时");
  act(() => result.current.toggle());
  expect(result.current.playing).toBe(true);
  expect(result.current.error).toBe("");
});
it("慢速解码期间不追加播放请求，帧到达后才继续，切换操作停止播放", async () => {
  vi.useFakeTimers();
  vi.mocked(recorderApi.videoTimeline).mockResolvedValue(overview);
  const { result, rerender } = renderHook(
    ({ action }) => usePlayback("session", action),
    { initialProps: { action: clickAction } },
  );
  await act(async () => {});
  act(() => result.current.toggle());
  const initial = result.current.seek;
  act(() => vi.advanceTimersByTime(2000));
  expect(result.current.seek).toEqual(initial);
  const frame: VideoFrame = {
    image: { hash: "a", path: "a", bytes: 1, size: [100, 100] },
    url: "image",
    at_qpc: timeQpc(overview, 0),
    presented_qpc: timeQpc(overview, 0),
    acquired_qpc: timeQpc(overview, 0),
    repeated: false,
    segment: "000001",
    sequence: 1,
    screen_origin: [0, 0],
    dpi: [96, 96],
    qpc_frequency: overview.frequency,
  };
  act(() => result.current.onFrame(frame, initial!.serial));
  act(() => vi.advanceTimersByTime(120));
  expect(result.current.seek?.serial).toBeGreaterThan(initial!.serial);
  rerender({ action: { ...clickAction, id: 99 } });
  expect(result.current.playing).toBe(false);
  expect(result.current.seek).toBeUndefined();
  rerender({ action: clickAction });
  expect(result.current.playing).toBe(false);
  act(() => result.current.request(1000, -1, 1));
  act(() =>
    result.current.onFrame(
      { ...frame, at_qpc: timeQpc(overview, 1250) },
      result.current.seek!.serial,
    ),
  );
  expect(result.current.time).toBe(1000);
  expect(result.current.seek).toMatchObject({
    qpc: timeQpc(overview, 1000),
    direction: -1,
    markerId: 1,
  });
});
