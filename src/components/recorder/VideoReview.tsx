import { useEffect, useRef, useState } from "react";
import { videoFrameLoader } from "../../features/recorder/video-loader";
import type { RecordingRecord, RawInput } from "../../features/recorder/model";
import { isPointerAction } from "../../features/recorder/video-target";
import type { VideoFrame, VideoSeek } from "../../features/recorder/video";
import { Button } from "../ui";
import { VideoImage } from "./VideoImage";

/** 每次请求只保留一帧；操作、目录或帧切换后丢弃迟到结果。 */
export function VideoReview({
  directory,
  action,
  point,
  resultQuery,
  seek,
  onFrame,
  onReadError,
  onManualSeek,
}: {
  readonly directory: string;
  readonly action: RecordingRecord | null;
  readonly point: RawInput["point"] | null;
  readonly seek?: VideoSeek;
  readonly onFrame?: (frame: VideoFrame, requestSerial?: number) => void;
  readonly onReadError?: (message: string, requestSerial?: number) => void;
  readonly onManualSeek?: () => void;
  readonly resultQuery?: {
    readonly qpc: string;
    readonly direction: number;
    readonly settleAfter?: string;
  };
}) {
  const interaction =
    action && "Interaction" in action.data ? action.data.Interaction : null;
  const baseline = String(
    interaction?.from_qpc ??
      (action && "Raw" in action.data
        ? action.data.Raw.qpc
        : action?.written_qpc) ??
      seek?.qpc ??
      "0",
  );
  const click = action ? isPointerAction(action) : false;
  const callbacks = useRef({ onFrame, onReadError });
  callbacks.current = { onFrame, onReadError };
  const [request, setRequest] = useState(() =>
    seek
      ? {
          qpc: seek.qpc,
          direction: seek.direction ?? 0,
          settleAfter: undefined as string | undefined,
          serial: 0,
          seekSerial: seek.serial,
          result: false,
          before: false,
        }
      : click
        ? {
            qpc: baseline,
            direction: -1,
            settleAfter: undefined as string | undefined,
            serial: 0,
            seekSerial: undefined as number | undefined,
            result: false,
            before: true,
          }
        : {
            qpc: baseline,
            direction: 0,
            ...resultQuery,
            serial: 0,
            seekSerial: undefined as number | undefined,
            result: !!resultQuery,
            before: false,
          },
  );
  const [result, setResult] = useState<{
    frame: VideoFrame | null;
    error: string;
    serial: number;
  }>({ frame: null, error: "", serial: -1 });
  const key = `${directory}:${action?.id ?? "timeline"}`;
  useEffect(() => {
    if (!seek) return;
    setRequest((previous) => ({
      qpc: seek.qpc,
      direction: seek.direction ?? 0,
      settleAfter: undefined,
      serial: previous.serial + 1,
      seekSerial: seek.serial,
      result: false,
      before: false,
    }));
  }, [seek?.qpc, seek?.serial, seek?.direction]);
  useEffect(() => {
    if (!resultQuery) return;
    setRequest((previous) =>
      previous.result &&
      (previous.qpc !== resultQuery.qpc ||
        previous.direction !== resultQuery.direction ||
        previous.settleAfter !== resultQuery.settleAfter)
        ? {
            ...resultQuery,
            serial: previous.serial + 1,
            seekSerial: undefined,
            result: true,
            before: false,
          }
        : previous,
    );
  }, [resultQuery?.qpc, resultQuery?.direction, resultQuery?.settleAfter]);
  useEffect(() => {
    let cancelled = false;
    const controller = new AbortController();
    void videoFrameLoader
      .load(
        {
          directory,
          qpc: request.qpc,
          direction: request.direction,
          ...(request.result
            ? { resultAnchor: baseline, settleAfter: request.settleAfter }
            : {}),
        },
        controller.signal,
      )
      .then(
        (frame) => {
          if (!cancelled) {
            setResult({ frame, error: "", serial: request.serial });
            callbacks.current.onFrame?.(frame, request.seekSerial);
          }
        },
        (error) => {
          if (!cancelled) {
            setResult({
              frame: null,
              error: String(error),
              serial: request.serial,
            });
            callbacks.current.onReadError?.(String(error), request.seekSerial);
          }
        },
      );
    return () => {
      cancelled = true;
      controller.abort();
    };
  }, [directory, key, request]);
  const busy = result.serial !== request.serial;
  const frame = busy ? null : result.frame;
  const load = (qpc: string, direction = 0, result = false, before = false) => {
    onManualSeek?.();
    setRequest((previous) => ({
      qpc,
      direction,
      serial: previous.serial + 1,
      seekSerial: undefined,
      result,
      before,
      settleAfter: result ? resultQuery?.settleAfter : undefined,
    }));
  };
  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col">
      {action && resultQuery && (
        <div className="flex flex-wrap items-center gap-2 border-b border-line px-4 py-2">
          <Button
            aria-pressed={request.before}
            disabled={busy}
            onClick={() => load(baseline, -1, false, true)}
          >
            操作时刻
          </Button>
          <Button
            aria-pressed={request.result}
            disabled={busy}
            onClick={() => load(resultQuery.qpc, resultQuery.direction, true)}
          >
            操作结果
          </Button>
        </div>
      )}
      <div className="flex min-h-0 flex-1 items-center justify-center overflow-auto bg-subtle p-3">
        {busy ? (
          <p role="status">正在读取视频帧…</p>
        ) : frame ? (
          <VideoImage frame={frame} point={request.before ? point : null} />
        ) : (
          <p role="alert" className="p-6 text-sm text-muted">
            {result.error}
          </p>
        )}
      </div>
    </div>
  );
}
