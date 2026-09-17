import type { RecordingRecord } from "../../../features/recorder/model";
import { useEffect, useState } from "react";
import { useReviewContext } from "../../../features/recorder/useReviewContext";
import { usePlayback } from "../../../features/recorder/usePlayback";
import { Button } from "../../ui";
import { EvidenceDetails } from "../EvidenceDetails";
import { PlaybackTimeline } from "./PlaybackTimeline";
import { VideoReview } from "../VideoReview";

/** 大画面和双轨时间线共用同一QPC时钟，不把任意游标位置伪装为某项操作。 */
export function PlaybackWorkspace({
  action,
  records,
  directory,
  frequency,
}: {
  readonly action: RecordingRecord;
  readonly records: readonly RecordingRecord[];
  readonly directory: string;
  readonly frequency: number;
}) {
  const playback = usePlayback(directory, action);
  const [selectionRevision, setSelectionRevision] = useState(0);
  const [selected, setSelected] = useState<number | null>(() =>
    "Interaction" in action.data
      ? (action.data.Interaction.raw[0] ?? null)
      : null,
  );
  const context = useReviewContext(directory, selected);
  const selectedRecords =
    selected === null
      ? records
      : context?.id === selected
        ? (context.data?.records ?? [])
        : [];
  const selectedAction =
    selected === null
      ? playback.seek
        ? undefined
        : action
      : context?.data?.action;
  useEffect(() => {
    if (!playback.seek) return;
    if (playback.seek.markerId !== undefined) {
      setSelected(playback.seek.markerId);
      return;
    }
    const markers = playback.timeline?.markers ?? [];
    let marker: (typeof markers)[number] | undefined;
    for (const value of markers) {
      if (value.time_ms > playback.time) break;
      marker = value;
    }
    setSelected(marker?.id ?? null);
  }, [playback.seek?.serial, playback.timeline, playback.time]);
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      {selectedAction ? (
        <EvidenceDetails
          key={selectionRevision}
          action={selectedAction}
          records={selectedRecords}
          directory={directory}
          frequency={frequency}
          seek={playback.seek}
          onFrame={playback.onFrame}
          onReadError={playback.onReadError}
          onManualSeek={playback.manual}
        />
      ) : (
        <div className="flex min-h-0 flex-1 flex-col">
          {playback.seek && (
            <VideoReview
              directory={directory}
              action={null}
              point={null}
              seek={playback.seek}
              onFrame={playback.onFrame}
              onReadError={playback.onReadError}
            />
          )}
          {selected !== null && (
            <p
              role={context?.error ? "alert" : "status"}
              className="shrink-0 p-3 text-xs text-muted"
            >
              {context?.id === selected && context.error
                ? context.error
                : "正在读取控件信息…"}
            </p>
          )}
        </div>
      )}
      {playback.error && (
        <p role="alert" className="p-2 text-xs text-warning">
          {playback.error}
          <Button onClick={playback.refresh}>重新加载时间线</Button>
        </p>
      )}
      {playback.timeline ? (
        <>
          {playback.timeline.tail && (
            <p role="status" className="px-3 text-xs text-warning">
              日志尾部尚未完整，显示已保存的输入。
            </p>
          )}
          <PlaybackTimeline
            timeline={playback.timeline}
            time={playback.time}
            playing={playback.playing}
            onSeek={playback.request}
            onMarker={(id) => {
              const marker = playback.timeline?.markers.find(
                (value) => value.id === Number(id),
              );
              if (!marker) return;
              playback.pause();
              playback.request(marker.time_ms, -1, marker.id);
              setSelected(marker.id);
              setSelectionRevision((value) => value + 1);
            }}
            onToggle={playback.toggle}
            onPause={playback.pause}
            onRefresh={playback.refresh}
          />
        </>
      ) : (
        !playback.error && (
          <p role="status" className="p-3 text-xs text-muted">
            正在读取录像时间线…
          </p>
        )
      )}
    </div>
  );
}
