import { useRef, useState, type PointerEvent } from 'react';
import { useTimelineScale } from './useTimelineScale';

/** 同一数值坐标系中的可编辑闭区间。 */
export type TimelineRange = Readonly<{ start: number; end: number }>;
/** 业务方提供标记内容，UI 不解释业务事件。 */
export type TimelineMarker = Readonly<{ id: string; value: number; label: string; lane?: number; tone?: 'neutral' | 'accent' | 'warm' }>;
/** 时间轴可使用时间或事件序号，格式化与步进由调用方决定。 */
export type TimelineProps = Readonly<{
  label: string;
  maximum: number;
  value: number;
  step?: number;
  valueText?: string;
  disabled?: boolean;
  markers: readonly TimelineMarker[];
  /** 轨道名称与 marker.lane 一一对应，调用方负责业务分类。 */
  lanes?: readonly string[];
  format: (value: number) => string;
  /** 需要辨认亚秒刻度时显示次刻度标签；序号轴无需开启。 */
  showMinorLabels?: boolean;
  onChange: (value: number) => void;
  selection?: Readonly<{
    /** 浏览模式保留选区和拖柄，仅在选择模式下拖动创建片段。 */
    creating: boolean;
    ranges: readonly TimelineRange[];
    active: number;
    onActive: (index: number) => void;
    onChange: (ranges: readonly TimelineRange[]) => void;
  }>;
}>;

/** 紧凑编辑时间轴：独立时间尺、操作轨道、播放游标和可触摸的范围拖柄。 */
export function Timeline({ label, maximum, value, step = 1, valueText, disabled = false, markers, lanes = ['事件'], format, showMinorLabels = false, onChange, selection }: TimelineProps) {
  const track = useRef<HTMLDivElement>(null);
  /** 拖动模式在按下时确定，后续渲染不能改变正在操作的对象。 */
  const drag = useRef<null | Readonly<{ type: 'seek' }> | Readonly<{ type: 'create'; start: number }> | Readonly<{ type: 'resize'; index: number; side: 'start' | 'end' }>>(null);
  const [draft, setDraft] = useState<TimelineRange | null>(null);
  const clamp = (next: number) => Math.max(0, Math.min(maximum, Math.round(next / step) * step));
  const percent = (next: number) => `${next / Math.max(1, maximum) * 100}%`;
  const point = (event: PointerEvent<HTMLElement>) => {
    const bounds = track.current?.getBoundingClientRect();
    return bounds?.width ? clamp((event.clientX - bounds.left) / bounds.width * maximum) : 0;
  };
  const resize = (index: number, side: 'start' | 'end', next: number) => {
    if (!selection) return;
    selection.onChange(selection.ranges.map((range, item) => item !== index ? range : side === 'start'
      ? { start: Math.min(next, range.end), end: range.end } : { start: range.start, end: Math.max(next, range.start) }));
    onChange(next);
  };
  const move = (next: number) => {
    const current = drag.current;
    if (current?.type === 'resize') resize(current.index, current.side, next);
    else if (current?.type === 'create') { setDraft({ start: Math.min(current.start, next), end: Math.max(current.start, next) }); onChange(next); }
    else if (current) onChange(next);
  };
  const ranges = selection?.ranges ?? [];
  const active = selection ? Math.min(selection.active, ranges.length - 1) : -1;
  const { subdivisions, ticks } = useTimelineScale(track, maximum, step);
  return (
    <div className="relative pb-[14px] pl-[66px] pr-3 pt-4">
      <div className="pointer-events-none absolute bottom-[14px] left-3 text-[11px] font-medium text-[#394458]">
        {lanes.map((lane) => <div key={lane} className="flex h-8 items-center">{lane}</div>)}
      </div>
      <div
        ref={track}
        className="relative touch-none select-none"
        style={{ height: 30 + lanes.length * 32 }}
        onPointerMove={(event) => move(point(event))}
        onPointerUp={(event) => {
          const current = drag.current;
          if (!current) return;
          const end = point(event);
          move(end);
          if (current.type === 'create' && selection && end !== current.start) {
            selection.onChange([...ranges, { start: Math.min(current.start, end), end: Math.max(current.start, end) }]);
            selection.onActive(ranges.length);
          }
          drag.current = null;
          setDraft(null);
        }}
        onPointerCancel={() => { drag.current = null; setDraft(null); }}
      >
        <div className="pointer-events-none absolute inset-x-0 top-0 h-7" aria-hidden="true">
          {ticks.map((tick, index) => (
            <span
              key={index}
              className={`absolute bottom-0 w-px ${index % subdivisions === 0 ? 'bg-[#8e99ac]' : index % subdivisions === subdivisions / 2 ? 'bg-[#b1bac9]' : 'bg-[#d2d9e5]'}`}
              style={{ left: percent(tick), height: index % subdivisions === 0 ? 14 : index % subdivisions === subdivisions / 2 ? 12 : 7 }}
            >
              {index % subdivisions === 0 || showMinorLabels ? <span className="absolute bottom-[17px] -translate-x-1/2 whitespace-nowrap text-[10px] font-normal tabular-nums text-[#707b8e]">{format(tick)}</span> : null}
            </span>
          ))}
        </div>
        {/* 原生 range 只负责浏览；片段拖柄是独立同级控件，不嵌套 slider。 */}
        <input
          type="range"
          aria-label={label}
          aria-valuetext={valueText ?? format(value)}
          min={0}
          max={maximum}
          step={step}
          value={value}
          disabled={disabled}
          onChange={(event) => onChange(Number(event.currentTarget.value))}
          className="absolute inset-x-0 top-0 z-10 h-8 w-full cursor-ew-resize opacity-0 focus-visible:opacity-20"
        />
        <div
          className="absolute inset-x-0 bottom-0"
          style={{ height: lanes.length * 32 }}
          onPointerDown={(event) => {
            if (disabled || event.button !== 0) return;
            const start = point(event);
            drag.current = selection?.creating ? { type: 'create', start } : { type: 'seek' };
            track.current?.setPointerCapture(event.pointerId);
            move(start);
          }}
        >
          {lanes.map((lane, index) => <div key={lane} className="pointer-events-none absolute -left-6 right-0 h-px bg-[#dfe3ec]" style={{ top: index * 32 + 16 }} />)}
          {markers.map((marker) => (
            <span
              key={marker.id}
              title={marker.label}
              className={`pointer-events-none absolute h-2.5 w-3 -translate-x-1/2 rounded-[2px] ${marker.tone === 'accent' ? 'bg-[#8b6bff]' : marker.tone === 'warm' ? 'bg-amber-400' : 'bg-[#a9afb9]'}`}
              style={{ left: percent(marker.value), top: (marker.lane ?? 0) * 32 + 11 }}
            />
          ))}
          {[...ranges, ...(draft ? [draft] : [])].map((range, index) => (
            <div
              key={index}
              className={`absolute inset-y-1 min-w-px rounded-sm ${index === active || index === ranges.length ? 'bg-[#3563ff]/12' : 'border border-slate-300 bg-slate-300/15'}`}
              style={{ left: percent(range.start), width: percent(range.end - range.start) }}
              onPointerDown={(event) => {
                if (disabled || event.button !== 0) return;
                event.stopPropagation();
                selection?.onActive(index);
                drag.current = { type: 'seek' };
                track.current?.setPointerCapture(event.pointerId);
                onChange(point(event));
              }}
            />
          ))}
        </div>
        <div className="pointer-events-none absolute -bottom-0.5 top-0 z-20 w-[1.5px] bg-[#ff7868]" style={{ left: percent(value) }} aria-hidden="true">
          <span className={`absolute -top-[22px] whitespace-nowrap rounded-lg bg-[#ff7868] px-2 py-0.5 text-[10px] leading-[13px] tabular-nums text-white ${value < maximum * 0.08 ? '' : value > maximum * 0.92 ? '-translate-x-full' : '-translate-x-1/2'}`}>{valueText ?? format(value)}</span>
        </div>
        {ranges[active] ? (['start', 'end'] as const).map((side) => (
          <div
            key={side}
            role="slider"
            aria-label={side === 'start' ? '片段起点' : '片段终点'}
            aria-valuemin={side === 'end' ? ranges[active].start : 0}
            aria-valuemax={side === 'start' ? ranges[active].end : maximum}
            aria-valuenow={ranges[active][side]}
            aria-valuetext={format(ranges[active][side])}
            tabIndex={disabled ? -1 : 0}
            title={format(ranges[active][side])}
            style={{ left: percent(ranges[active][side]), height: lanes.length * 32 - 8 }}
            className="absolute bottom-1 z-30 flex w-4 -translate-x-1/2 cursor-ew-resize items-center justify-center rounded-sm focus-visible:outline-2 focus-visible:outline-blue-500"
            onPointerDown={(event) => {
              if (disabled || event.button !== 0) return;
              drag.current = { type: 'resize', index: active, side };
              track.current?.setPointerCapture(event.pointerId);
            }}
            onKeyDown={(event) => {
              if (disabled) return;
              const next = event.key === 'ArrowLeft' ? ranges[active][side] - step : event.key === 'ArrowRight' ? ranges[active][side] + step : null;
              if (next !== null) { event.preventDefault(); resize(active, side, clamp(next)); }
            }}
          >
            <span className={`flex h-full w-2 items-center justify-center gap-px bg-[#3563ff] ${side === 'start' ? 'rounded-l-[4px] border-r border-white' : 'rounded-r-[4px] border-l border-white'}`}>
              <span className="h-2.5 w-px bg-white/90" />
              <span className="h-2.5 w-px bg-white/90" />
            </span>
          </div>
        )) : null}
      </div>
    </div>
  );
}
