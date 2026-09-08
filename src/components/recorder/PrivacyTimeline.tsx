import { useMemo, useState, type ReactNode } from 'react';
import Play from 'lucide-react/dist/esm/icons/play.mjs';
import Pause from 'lucide-react/dist/esm/icons/pause.mjs';
import { durationLabel, preciseDurationLabel, eventLabel, type PrivacyTimeRange, type RawTraceEvent } from '../../features/recorder';
import { Button, IconButton, Timeline } from '../ui';
import { TimelineRangeControls } from './TimelineRangeControls';
import { TimelineEventSummary } from './TimelineEventSummary';

/** 底部回看与片段选择轴；拖动浏览或多次拖动选择时间段，无需组合键。 */
export function PrivacyTimeline({ events, duration, time, playing, disabled, ranges, onSeek, onToggle, onRanges, actions }: Readonly<{
  /** 业务处理操作放入设计稿底栏，与范围选择并列。 */
  actions?: ReactNode;
  events: readonly RawTraceEvent[];
  duration: number;
  time: number;
  playing: boolean;
  disabled: boolean;
  ranges: readonly PrivacyTimeRange[];
  onSeek: (time: number) => void;
  onToggle: () => void;
  onRanges: (ranges: readonly PrivacyTimeRange[]) => void;
}>) {
  const [selecting, setSelecting] = useState(false);
  const [active, setActive] = useState(0);
  const markers = useMemo(() => {
    /** 每条轨道独立聚合，避免同一时间的鼠标事件覆盖键盘事件。 */
    const columns = new Map<string, RawTraceEvent>();
    for (const event of events) {
      const column = Math.round(event.elapsed_ms / Math.max(1, duration) * 1000);
      const lane = event.input.type === 'key' || event.input.type === 'clipboard' ? 0 : 1;
      columns.set(`${lane}:${column}`, event);
    }
    return [...columns.values()].map((event) => ({ id: String(event.sequence), value: event.elapsed_ms, label: eventLabel(event.input), lane: event.input.type === 'key' || event.input.type === 'clipboard' ? 0 : 1, tone: event.input.type === 'key' || event.input.type === 'clipboard' ? 'accent' as const : 'neutral' as const }));
  }, [events, duration]);
  return (
    <footer className="shrink-0 rounded-lg border border-[#dfe3ec] bg-[#fdfdff]">
      <div className="flex min-h-[52px] flex-wrap items-center gap-3 border-b border-[#dfe3ec] px-3 py-2.5">
        <IconButton
          icon={playing ? Pause : Play}
          label={playing ? '暂停' : '播放'}
          variant="primary"
          size="standard"
          className="!h-8 !w-10 !rounded-md !border-[#3563ff] !bg-[#3563ff] shadow-none"
          iconClassName="size-4 fill-current"
          disabled={disabled || !duration}
          onClick={onToggle}
        />
        <span className="mr-auto whitespace-nowrap text-[15px] font-medium tabular-nums text-[#283348]">
          {preciseDurationLabel(time)} / {preciseDurationLabel(duration)}
        </span>
        <div className="flex rounded-md bg-[#f3f4f8]">
          <Button
            disabled={disabled}
            aria-pressed={!selecting}
            variant={selecting ? 'ghost' : 'selected'}
            className="min-w-24 !rounded-md shadow-none"
            onClick={() => { setSelecting(false); onSeek(time); }}
          >浏览画面</Button>
          <Button
            disabled={disabled || !duration}
            aria-pressed={selecting}
            variant={selecting ? 'selected' : 'ghost'}
            className="min-w-28 !rounded-md shadow-none"
            onClick={() => {
              onSeek(time);
              setSelecting(true);
              if (!ranges.length) {
                onRanges([{ start: Math.min(time, Math.max(0, duration - 2000)), end: Math.min(duration, time + 2000) }]);
                setActive(0);
              }
            }}
          >选择时间段</Button>
        </div>
      </div>
      <TimelineEventSummary events={events} time={time} />
      <Timeline
        label="录制时间轴"
        maximum={duration}
        value={time}
        step={1}
        valueText={preciseDurationLabel(time)}
        disabled={disabled}
        markers={markers}
        lanes={['键盘', '鼠标']}
        format={durationLabel}
        onChange={onSeek}
        selection={{ ranges, active, creating: selecting, onActive: setActive, onChange: onRanges }}
      />
      {ranges.length > 0 ? (
        <div className="flex min-h-11 flex-wrap items-center gap-2.5 border-t border-[#dfe3ec] px-3 py-2">
          <TimelineRangeControls ranges={ranges} active={Math.min(active, ranges.length - 1)} duration={duration} time={time} disabled={disabled} onActive={setActive} onChange={onRanges} />
          {actions}
        </div>
      ) : null}
    </footer>
  );
}
