import { Button, IconButton, Input } from '../ui';
import X from 'lucide-react/dist/esm/icons/x.mjs';
import SlidersHorizontal from 'lucide-react/dist/esm/icons/sliders-horizontal.mjs';
import { useState } from 'react';
import { preciseDurationLabel, type PrivacyTimeRange } from '../../features/recorder';

/** 显式编辑选中片段，秒输入支持毫秒精度；不依赖精细拖动。 */
export function TimelineRangeControls({ ranges, active, duration, time, disabled, onActive, onChange }: Readonly<{
  ranges: readonly PrivacyTimeRange[];
  active: number;
  duration: number;
  time: number;
  disabled: boolean;
  onActive: (index: number) => void;
  onChange: (ranges: readonly PrivacyTimeRange[]) => void;
}>) {
  const range = ranges[active];
  const [adjusting, setAdjusting] = useState(false);
  /** 移除任意标签后保留原选中片段身份；移除当前片段则选择相邻片段。 */
  const remove = (index: number) => {
    onChange(ranges.filter((_, item) => item !== index));
    onActive(index < active ? active - 1 : index === active ? Math.max(0, Math.min(active, ranges.length - 2)) : active);
  };
  const update = (side: 'start' | 'end', value: number) => {
    if (!range || !Number.isFinite(value)) return;
    const bounded = Math.max(0, Math.min(duration, Math.round(value)));
    const next = side === 'start' ? { start: Math.min(bounded, range.end), end: range.end } : { start: range.start, end: Math.max(bounded, range.start) };
    onChange(ranges.map((item, index) => index === active ? next : item));
  };
  return (
    <div className="flex min-w-0 flex-[1_1_360px] flex-wrap items-center gap-3">
      <span className="shrink-0 text-xs text-[#394458]">已选择时间段</span>
      <div className="flex min-w-0 flex-1 gap-2 overflow-x-auto">
        {ranges.map((item, index) => (
          <div
            key={index}
            className={`flex h-[27px] shrink-0 items-center overflow-hidden rounded-md border ${index === active ? 'border-[#7d99ff] bg-[#f8faff]' : 'border-slate-200 bg-white'}`}
          >
            <Button
              size="compact"
              disabled={disabled}
              variant="ghost"
              aria-pressed={index === active}
              className="rounded-none border-0 !pr-2 !pl-3 tabular-nums !text-[#394458]"
              onClick={() => onActive(index)}
            >{preciseDurationLabel(item.start)} – {preciseDurationLabel(item.end)}</Button>
            <IconButton
              icon={X}
              variant="ghost"
              label={`取消选择片段 ${index + 1}`}
              disabled={disabled}
              className="rounded-none border-0"
              onClick={() => remove(index)}
            />
          </div>
        ))}
      </div>
      {range ? (
        <>
          <Button
            size="compact"
            className="!h-7 min-w-[104px] !rounded-md shadow-none"
            disabled={disabled}
            aria-expanded={adjusting}
            onClick={() => setAdjusting((value) => !value)}
          >
            <SlidersHorizontal className="mr-1 size-3.5" />
            {adjusting ? '收起调整' : '调整时间'}
          </Button>
          {adjusting ? <>
          <label className="flex items-center gap-1 text-xs text-slate-600">
            开始（秒）
            <Input
              aria-label="片段起点（秒）"
              type="number"
              min={0}
              max={range.end / 1000}
              step={0.001}
              value={range.start / 1000}
              disabled={disabled}
              containerClassName="w-24"
              onChange={(event) => update('start', event.currentTarget.valueAsNumber * 1000)}
            />
          </label>
          <label className="flex items-center gap-1 text-xs text-slate-600">
            结束（秒）
            <Input
              aria-label="片段终点（秒）"
              type="number"
              min={range.start / 1000}
              max={duration / 1000}
              step={0.001}
              value={range.end / 1000}
              disabled={disabled}
              containerClassName="w-24"
              onChange={(event) => update('end', event.currentTarget.valueAsNumber * 1000)}
            />
          </label>
          </> : null}
        </>
      ) : null}
    </div>
  );
}
