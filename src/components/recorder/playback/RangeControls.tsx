import { useState } from "react";
import { Button, Input, type TimelineRange } from "../../ui";
import { playbackTime } from "../../../features/recorder/playback";

/** 仅在选择模式挂载；隐藏时不会留下可聚焦的隐私控件。 */
export function RangeControls({
  ranges,
  active,
  duration,
  onActive,
  onChange,
}: {
  readonly ranges: readonly TimelineRange[];
  readonly active: number;
  readonly duration: number;
  readonly onActive: (index: number) => void;
  readonly onChange: (ranges: readonly TimelineRange[]) => void;
}) {
  const [adjusting, setAdjusting] = useState(false);
  const range = ranges[active];
  const update = (side: "start" | "end", value: number) => {
    if (!range || !Number.isFinite(value)) return;
    const bounded = Math.max(0, Math.min(duration, Math.round(value * 1000)));
    onChange(
      ranges.map((r, i) =>
        i !== active
          ? r
          : side === "start"
            ? { start: Math.min(bounded, r.end), end: r.end }
            : { start: r.start, end: Math.max(bounded, r.start) },
      ),
    );
  };
  return (
    <div
      className="flex flex-wrap items-center gap-2 border-t border-line px-3 py-2"
      aria-label="时间段处理"
    >
      <span className="text-xs text-muted">已选择时间段</span>
      <div className="flex min-w-0 flex-1 gap-2 overflow-x-auto">
        {ranges.map((r, index) => (
          <div
            key={index}
            className="flex shrink-0 items-center rounded-md border border-line"
          >
            <Button
              variant="ghost"
              aria-pressed={index === active}
              onClick={() => onActive(index)}
            >
              {playbackTime(r.start)} – {playbackTime(r.end)}
            </Button>
            <Button
              variant="ghost"
              aria-label={`取消选择片段 ${index + 1}`}
              onClick={() => {
                onChange(ranges.filter((_, i) => i !== index));
                onActive(Math.max(0, active - (index <= active ? 1 : 0)));
              }}
            >
              ×
            </Button>
          </div>
        ))}
        {!range && (
          <span className="self-center text-xs text-muted">
            在时间线上拖动，选择需要处理的片段
          </span>
        )}
      </div>
      <Button
        disabled={!range}
        aria-expanded={adjusting}
        onClick={() => setAdjusting((v) => !v)}
      >
        调整时间
      </Button>
      <Button disabled title="视频、输入记录与导出的隐私处理将在后续接入">
        处理所选内容
      </Button>
      {adjusting && range && (
        <div className="flex w-full items-center gap-3 text-xs">
          <label>
            开始（秒）
            <Input
              className="ml-2 w-28"
              aria-label="片段起点（秒）"
              type="number"
              min={0}
              max={range.end / 1000}
              step={0.001}
              value={range.start / 1000}
              onChange={(e) => update("start", e.currentTarget.valueAsNumber)}
            />
          </label>
          <label>
            结束（秒）
            <Input
              className="ml-2 w-28"
              aria-label="片段终点（秒）"
              type="number"
              min={range.start / 1000}
              max={duration / 1000}
              step={0.001}
              value={range.end / 1000}
              onChange={(e) => update("end", e.currentTarget.valueAsNumber)}
            />
          </label>
        </div>
      )}
      <p className="w-full text-[11px] text-muted">
        隐私处理尚未接入。当前选择不会修改录像、输入记录或导出内容。
      </p>
    </div>
  );
}
