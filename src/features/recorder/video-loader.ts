import { recorderApi } from "./api";
import type { VideoFrame } from "./video";

interface Query {
  readonly directory: string;
  readonly qpc: string;
  readonly direction: number;
  readonly resultAnchor?: string;
  readonly settleAfter?: string;
}
interface Subscriber {
  readonly resolve: (frame: VideoFrame) => void;
  readonly reject: (error: unknown) => void;
  readonly dispose: () => void;
}
interface Job {
  readonly key: string;
  readonly query: Query;
  readonly subscribers: Set<Subscriber>;
}
const cancelled = () => new DOMException("取帧请求已被替换", "AbortError");

/** 一个在途解码和一个最新待处理请求；重复挂载订阅同一在途任务。 */
export class VideoFrameLoader {
  private active: Job | null = null;
  private pending: Job | null = null;

  constructor(private readonly read: (query: Query) => Promise<VideoFrame>) {}

  load(query: Query, signal: AbortSignal): Promise<VideoFrame> {
    if (signal.aborted) return Promise.reject(cancelled());
    const key = JSON.stringify(query);
    let job =
      this.active?.key === key
        ? this.active
        : this.pending?.key === key
          ? this.pending
          : null;
    if (!job) {
      if (this.pending) this.settle(this.pending, { error: cancelled() });
      job = { key, query, subscribers: new Set() };
      this.pending = job;
    }
    const selected = job;
    const result = new Promise<VideoFrame>((resolve, reject) => {
      const abort = () => {
        selected.subscribers.delete(subscriber);
        reject(cancelled());
        if (this.pending === selected && !selected.subscribers.size)
          this.pending = null;
      };
      const subscriber: Subscriber = {
        resolve,
        reject,
        dispose: () => signal.removeEventListener("abort", abort),
      };
      selected.subscribers.add(subscriber);
      signal.addEventListener("abort", abort, { once: true });
    });
    this.pump();
    return result;
  }

  private pump() {
    if (this.active || !this.pending) return;
    const job = this.pending;
    this.pending = null;
    this.active = job;
    try {
      void this.read(job.query).then(
        (frame) => this.finish(job, { frame }),
        (error) => this.finish(job, { error }),
      );
    } catch (error) {
      this.finish(job, { error });
    }
  }

  private finish(job: Job, result: { frame: VideoFrame } | { error: unknown }) {
    this.settle(job, result);
    this.active = null;
    this.pump();
  }

  private settle(job: Job, result: { frame: VideoFrame } | { error: unknown }) {
    for (const subscriber of job.subscribers) {
      subscriber.dispose();
      if ("frame" in result) subscriber.resolve(result.frame);
      else subscriber.reject(result.error);
    }
    job.subscribers.clear();
  }
}

export const videoFrameLoader = new VideoFrameLoader(
  ({ directory, qpc, direction, resultAnchor, settleAfter }) =>
    recorderApi.videoFrame(
      directory,
      qpc,
      direction,
      resultAnchor ?? null,
      settleAfter ?? null,
    ),
);
