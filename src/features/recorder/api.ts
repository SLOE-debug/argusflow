import { invoke } from "@tauri-apps/api/core";
import type { VideoFrame } from "./video";
import type { VideoTimeline } from "./playback";
import type {
  Attachment,
  Cursor,
  JournalPage,
  Phase,
  RecorderStatus,
  RecordingEntry,
  Session,
  OperationContext,
} from "./model";
/** IPC 适配层不持有 UI 状态。 */
export const recorderApi = {
  context: (directory: string, rawId: number) =>
    invoke<OperationContext>("recorder_context", {
      directory,
      rawId,
    }),
  videoTimeline: (directory: string) =>
    invoke<VideoTimeline>("recorder_video_timeline", { directory }),
  videoFrame: (
    directory: string,
    qpc: string,
    direction: number = 0,
    resultAnchor: string | null = null,
    settleAfter: string | null = null,
  ) =>
    invoke<VideoFrame>("recorder_video_frame", {
      directory,
      qpc,
      direction,
      resultAnchor,
      settleAfter,
    }),
  start: () => invoke<string>("recorder_start"),
  status: () => invoke<RecorderStatus>("recorder_status"),
  transition: (phase: Phase) => invoke<void>("recorder_transition", { phase }),
  list: () => invoke<readonly RecordingEntry[]>("recorder_list"),
  open: (directory: string) =>
    invoke<Session>("recorder_session", { directory }),
  read: (directory: string, cursor: Cursor) =>
    invoke<JournalPage>("recorder_read", { directory, cursor }),
  image: (directory: string, image: Attachment) =>
    invoke<string>("recorder_image", { directory, image }),
};
