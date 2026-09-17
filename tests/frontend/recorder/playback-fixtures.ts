import type { VideoTimeline } from "../../../src/features/recorder/playback";
export const overview: VideoTimeline = {
  origin_qpc: "9007199254741000",
  frequency: 10_000_000,
  duration_ms: 10000,
  segments: [{ start_ms: 0, end_ms: 10000 }],
  tail: null,
  markers: [
    { id: 1, time_ms: 1000, input: { kind: "key", vk: 78 } },
    { id: 2, time_ms: 1000, input: { kind: "button", button: "Left" } },
  ],
};
