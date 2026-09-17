import { useEffect, useRef, useState } from "react";
import { recorderApi } from "./api";
import { qpcTime, timeQpc, type VideoTimeline } from "./playback";
import type { RecordingRecord } from "./model";
import type { VideoFrame, VideoSeek } from "./video";

/** 只读元数据与单帧回看；下一次播放取帧等当前结果返回，避免堆积。 */
export function usePlayback(directory: string, action: RecordingRecord) {
  const [timeline, setTimeline] = useState<VideoTimeline | null>(null);
  const [error, setError] = useState("");
  const [revision, setRevision] = useState(0);
  const [position, setPosition] = useState<{
    action: number;
    time: number;
    seek?: VideoSeek;
  } | null>(null);
  const [playingFor, setPlayingFor] = useState<number | null>(null);
  const [completed, setCompleted] = useState(0);
  const serial = useRef(0);
  const activeSeek = useRef<number | undefined>(undefined);
  const pending = useRef(false);
  const clock = useRef({ started: 0, from: 0 });
  const currentAction = useRef(action.id);
  currentAction.current = action.id;
  useEffect(() => {
    setPlayingFor(null);
    activeSeek.current = undefined;
  }, [action.id]);
  useEffect(() => {
    let cancelled = false;
    setError("");
    void recorderApi.videoTimeline(directory).then(
      (value) => {
        if (!cancelled) setTimeline(value);
      },
      (failure) => {
        if (!cancelled) setError(String(failure));
      },
    );
    return () => {
      cancelled = true;
    };
  }, [directory, revision]);
  const initial = String(
    "Interaction" in action.data
      ? action.data.Interaction.from_qpc
      : action.written_qpc,
  );
  const time = Math.max(
    0,
    Math.min(
      timeline?.duration_ms ?? 0,
      position?.action === action.id
        ? position.time
        : timeline
          ? qpcTime(timeline, initial)
          : 0,
    ),
  );
  const seek = position?.action === action.id ? position.seek : undefined;
  const playing = playingFor === action.id;
  const request = (next: number, direction = 0, markerId?: number) => {
    if (!timeline) return;
    setError("");
    // 区间终点是排他边界；最后一个tick仍落在已保存的视频内。
    const bounded = Math.max(
      0,
      Math.min(
        next,
        Math.max(0, timeline.duration_ms - 1000 / timeline.frequency),
      ),
    );
    pending.current = true;
    activeSeek.current = ++serial.current;
    setPosition({
      action: action.id,
      time: bounded,
      seek: {
        qpc: timeQpc(timeline, bounded),
        serial: activeSeek.current,
        direction,
        markerId,
      },
    });
  };
  useEffect(() => {
    if (!playing || !timeline || pending.current) return;
    const timer = setTimeout(() => {
      let next = clock.current.from + performance.now() - clock.current.started;
      // 播放越过未录制间隔；手动拖动仍允许查看该时间点的明确缺口提示。
      const segment = timeline.segments.find((s) => s.end_ms > next);
      if (segment) next = Math.max(next, segment.start_ms);
      if (next >= timeline.duration_ms) {
        setPlayingFor(null);
        request(timeline.duration_ms);
      } else request(next);
    }, 120);
    return () => clearTimeout(timer);
  }, [playing, completed, timeline]);
  const onFrame = (frame: VideoFrame, requestSerial?: number) => {
    if (currentAction.current !== action.id) return;
    if (requestSerial !== activeSeek.current) return;
    pending.current = false;
    if (timeline)
      setPosition((previous) =>
        previous?.action === action.id && previous.seek
          ? previous
          : { action: action.id, time: qpcTime(timeline, frame.at_qpc) },
      );
    setCompleted((value) => value + 1);
  };
  return {
    timeline,
    error,
    time,
    seek,
    playing,
    onFrame,
    onReadError: (message: string, requestSerial?: number) => {
      if (requestSerial !== activeSeek.current) return;
      pending.current = false;
      setPlayingFor(null);
      setError(message);
    },
    refresh: () => {
      setPlayingFor(null);
      setRevision((value) => value + 1);
    },
    pause: () => setPlayingFor(null),
    manual: () => {
      setPlayingFor(null);
      activeSeek.current = undefined;
      setPosition(null);
    },
    request,
    toggle: () => {
      if (playing) {
        setPlayingFor(null);
        return;
      }
      if (!timeline) return;
      const candidate =
        time >= timeline.duration_ms - 1
          ? (timeline.segments[0]?.start_ms ?? 0)
          : time;
      const segment = timeline.segments.find(
        (value) => value.end_ms > candidate,
      );
      if (!segment) {
        setError("没有可播放的视频片段");
        return;
      }
      // 启动与连续播放使用相同的片段边界；不能先请求缺口再等待自动跳过。
      const from = Math.max(candidate, segment.start_ms);
      clock.current = { started: performance.now(), from };
      setPlayingFor(action.id);
      request(from);
    },
  };
}
