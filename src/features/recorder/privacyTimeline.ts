import type { RawTraceEvent } from './model';
import type { ScreenshotEvidence } from './inspectionContracts';
import type { PrivacyEdit, PrivacyImageKind } from './privacy';

/** 用户选择的闭区间，单位为录制相对毫秒。 */
export type PrivacyTimeRange = Readonly<{ start: number; end: number }>;
/** 已落盘的屏幕采样，使用采样时间而非所属输入的时间播放。 */
export type PrivacyFrame = Readonly<{ sequence: number; kind: PrivacyImageKind; shot: ScreenshotEvidence }>;
/** 片段处理的有限选项。 */
export type PrivacyTreatment = 'everything' | 'inputs' | 'screenshots';

/** 整理采样索引，按真实采样时间排序。 */
export function privacyFrames(events: readonly RawTraceEvent[]): readonly PrivacyFrame[] {
  return events.flatMap((event) => {
    const frames: PrivacyFrame[] = [];
    if (event.evidence?.click_target) frames.push({ sequence: event.sequence, kind: 'target', shot: event.evidence.click_target });
    if (event.evidence?.screenshot) frames.push({ sequence: event.sequence, kind: 'window', shot: event.evidence.screenshot });
    return frames;
  }).sort((a, b) => a.shot.captured_at_ms - b.shot.captured_at_ms);
}

/** 二分查找当前或此前最后一个时间点；首个时间点之前不展示未来画面。 */
export function latestAt<T>(items: readonly T[], time: number, timestamp: (item: T) => number): T | undefined {
  let low = 0;
  let high = items.length;
  while (low < high) {
    const middle = Math.floor((low + high) / 2);
    if (timestamp(items[middle]) <= time) low = middle + 1;
    else high = middle;
  }
  return items[low - 1];
}

/** 合并重叠片段，避免重复处理同一个截图。 */
export function mergePrivacyRanges(ranges: readonly PrivacyTimeRange[]): readonly PrivacyTimeRange[] {
  const merged: { start: number; end: number }[] = [];
  for (const range of [...ranges].sort((a, b) => a.start - b.start)) {
    const previous = merged.at(-1);
    if (previous && range.start <= previous.end) previous.end = Math.max(previous.end, range.end);
    else merged.push({ ...range });
  }
  return merged;
}

/** 选择按事件持续区间求交；屏幕按实际展示区间求交，包含延续到片段内的上一帧。 */
export function timelinePrivacyEdits(events: readonly RawTraceEvent[], frames: readonly PrivacyFrame[], ranges: readonly PrivacyTimeRange[], treatment: PrivacyTreatment, duration: number): readonly PrivacyEdit[] {
  const overlaps = (start: number, end: number) => ranges.some((range) => start <= range.end && end >= range.start);
  if (treatment === 'screenshots') {
    return frames.filter((frame, index) => overlaps(frame.shot.captured_at_ms, frame.shot.captured_at_ms)
      || overlaps(frame.shot.captured_at_ms, (frames[index + 1]?.shot.captured_at_ms ?? duration + 1) - 1))
      .map((frame) => ({ type: 'mosaic', sequence: frame.sequence, kind: frame.kind, rect: { x: 0, y: 0, width: frame.shot.width, height: frame.shot.height } }));
  }
  return events.filter((event) => overlaps(event.elapsed_ms, event.input.type === 'pointer_motion' ? event.input.ended_ms : event.elapsed_ms))
    .filter((event) => treatment === 'everything' || event.input.type === 'key' || event.input.type === 'clipboard')
    .map((event) => ({ type: 'erase_event', sequence: event.sequence }));
}
