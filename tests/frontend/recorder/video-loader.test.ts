import { describe, expect, it, vi } from "vitest";
import { VideoFrameLoader } from "../../../src/features/recorder/video-loader";
import type { VideoFrame } from "../../../src/features/recorder/video";

const frame = { sequence: 1 } as VideoFrame;
const query = (qpc: string) => ({ directory: "recording", qpc, direction: 1 });
const flush = () => Promise.resolve();

describe("视频取帧调度", () => {
  it("快速切换只执行当前解码和最后一个待处理请求", async () => {
    let resolve!: (frame: VideoFrame) => void;
    const read = vi
      .fn()
      .mockImplementationOnce(
        () =>
          new Promise((r) => {
            resolve = r;
          }),
      )
      .mockResolvedValue(frame);
    const loader = new VideoFrameLoader(read);
    const first = loader.load(query("1"), new AbortController().signal);
    const skipped = loader
      .load(query("2"), new AbortController().signal)
      .catch((e) => e.name);
    const latest = loader.load(query("3"), new AbortController().signal);
    expect(await skipped).toBe("AbortError");
    expect(read).toHaveBeenCalledTimes(1);
    resolve(frame);
    expect(await first).toBe(frame);
    expect(await latest).toBe(frame);
    expect(read.mock.calls.map((c) => c[0].qpc)).toEqual(["1", "3"]);
  });

  it("取消订阅不冒充取消原生解码，相同请求可再次订阅", async () => {
    let resolve!: (frame: VideoFrame) => void;
    const read = vi.fn(
      () =>
        new Promise<VideoFrame>((r) => {
          resolve = r;
        }),
    );
    const loader = new VideoFrameLoader(read);
    const control = new AbortController();
    const first = loader.load(query("1"), control.signal).catch((e) => e.name);
    control.abort();
    const second = loader.load(query("1"), new AbortController().signal);
    resolve(frame);
    expect(await first).toBe("AbortError");
    expect(await second).toBe(frame);
    expect(read).toHaveBeenCalledTimes(1);
  });

  it("错误释放当前任务，已取消的等待请求不会执行", async () => {
    let reject!: (error: Error) => void;
    const read = vi
      .fn()
      .mockImplementationOnce(
        () =>
          new Promise((_, r) => {
            reject = r;
          }),
      )
      .mockResolvedValue(frame);
    const loader = new VideoFrameLoader(read);
    const first = loader
      .load(query("1"), new AbortController().signal)
      .catch((e) => e.message);
    const control = new AbortController();
    const pending = loader
      .load(query("2"), control.signal)
      .catch((e) => e.name);
    control.abort();
    reject(Error("read failed"));
    expect(await pending).toBe("AbortError");
    expect(await first).toBe("read failed");
    await flush();
    expect(await loader.load(query("3"), new AbortController().signal)).toBe(
      frame,
    );
    expect(read.mock.calls.map((c) => c[0].qpc)).toEqual(["1", "3"]);
  });
});
