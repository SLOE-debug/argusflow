import { useMemo, useState } from "react";
import { Play, Pause, RefreshCw } from "lucide-react";
import { Button, IconButton, Timeline, type TimelineRange } from "../../ui";
import {
  playbackTime,
  timelineMarkers,
  type VideoTimeline,
} from "../../../features/recorder/playback";
import { RangeControls } from "./RangeControls";

/** 浏览仅移动游标；选择模式独占选区、拖柄和处理底栏。 */
export function PlaybackTimeline({
  timeline,
  time,
  playing,
  onSeek,
  onToggle,
  onPause,
  onRefresh,
  onMarker,
}: {
  readonly timeline: VideoTimeline;
  readonly time: number;
  readonly playing: boolean;
  readonly onSeek: (time: number) => void;
  readonly onToggle: () => void;
  readonly onPause: () => void;
  readonly onRefresh: () => void;
  readonly onMarker: (id: string) => void;
}) {
  const [selecting, setSelecting] = useState(false);
  const [ranges, setRanges] = useState<readonly TimelineRange[]>([]);
  const [active, setActive] = useState(0);
  const markers = useMemo(() => timelineMarkers(timeline), [timeline]);
  const duration = timeline.duration_ms;
  const seek = (next: number) => {
    onPause();
    onSeek(next);
  };
  return (
    <footer
      className="shrink-0 border-t border-line bg-surface"
      aria-label="录像时间线"
    >
      <div className="flex flex-wrap items-center gap-3 border-b border-line px-3 py-2">
        <IconButton
          aria-label={playing ? "暂停回看" : "播放回看"}
          disabled={duration <= 0}
          onClick={() => {
            setSelecting(false);
            onToggle();
          }}
        >
          {playing ? <Pause size={16} /> : <Play size={16} />}
        </IconButton>
        <span className="mr-auto text-sm tabular-nums">
          {playbackTime(time)} / {playbackTime(duration)}
        </span>
        <IconButton aria-label="刷新录像时间线" onClick={onRefresh}>
          <RefreshCw size={14} />
        </IconButton>
        <div className="flex rounded-md bg-subtle">
          <Button
            variant="ghost"
            aria-pressed={!selecting}
            className={!selecting ? "bg-accent-soft text-accent" : ""}
            onClick={() => {
              onPause();
              setSelecting(false);
            }}
          >
            浏览画面
          </Button>
          <Button
            variant="ghost"
            disabled={!duration}
            aria-pressed={selecting}
            className={selecting ? "bg-accent-soft text-accent" : ""}
            onClick={() => {
              onPause();
              setSelecting(true);
              if (!ranges.length) {
                setRanges([
                  {
                    start: Math.min(time, Math.max(0, duration - 2000)),
                    end: Math.min(duration, time + 2000),
                  },
                ]);
                setActive(0);
              }
            }}
          >
            选择时间段
          </Button>
        </div>
      </div>
      <Timeline
        key={selecting ? "select" : "browse"}
        label="录制时间轴"
        maximum={duration}
        value={time}
        step={1}
        valueText={playbackTime(time)}
        disabled={!duration}
        markers={markers}
        lanes={["键盘", "鼠标"]}
        format={(value) =>
          value % 1000 ? playbackTime(value) : playbackTime(value).slice(0, 5)
        }
        onChange={seek}
        onMarker={onMarker}
        selection={
          selecting
            ? {
                ranges,
                active,
                creating: true,
                onActive: setActive,
                onChange: (next) => {
                  onPause();
                  setRanges(next.slice(0, 64));
                },
              }
            : undefined
        }
      />
      {selecting && (
        <RangeControls
          ranges={ranges}
          active={Math.min(active, ranges.length - 1)}
          duration={duration}
          onActive={setActive}
          onChange={setRanges}
        />
      )}
    </footer>
  );
}
