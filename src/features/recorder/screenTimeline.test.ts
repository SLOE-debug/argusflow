import { describe, expect, it } from 'vitest';
import { screenPrivacyFrames, timelinePrivacyEdits } from './privacyTimeline';
import type { ScreenTimeline } from './screenContracts';

const bounds = { x: -1920, y: 0, width: 1920, height: 1080 };
const screen: ScreenTimeline = {
  duration_us: 20000,
  refinement: { state: 'complete' },
  completeness: { state: 'complete' },
  diagnostics: { queue_peak_items: 2, queue_peak_bytes: 16, readback_bytes: 32, diff_total_us: 1 },
  frames: [
    { id: 1, source: 1, generation: 1, revision: 1, presented_us: 0, frozen_us: 10, bounds, previous: null, changes: [bounds], patches: [{ index: 0, bounds }] },
    { id: 2, source: 2, generation: 1, revision: 1, presented_us: 5000, frozen_us: 5010, bounds, previous: null, changes: [bounds], patches: [{ index: 0, bounds }] },
    { id: 3, source: 1, generation: 1, revision: 2, presented_us: 20000, frozen_us: 20010, bounds, previous: 1, changes: [{ x: -1919, y: 1, width: 1, height: 1 }], patches: [{ index: 0, bounds }] },
  ],
};

describe('独立屏幕时间线', () => {
  it('无输入事件仍能回放，负屏幕坐标转换到帧内区域', () => {
    const frames = screenPrivacyFrames(screen);
    expect(frames.map((frame) => frame.sequence)).toEqual([1, 2, 3]);
    expect(frames[2].changes).toEqual([{ x: 1, y: 1, width: 1, height: 1 }]);
  });
  it('隐私范围同时覆盖各来源延续到区间内的帧', () => {
    const edits = timelinePrivacyEdits([], screenPrivacyFrames(screen), [{ start: 10, end: 15 }], 'everything', 30);
    expect(edits.map((edit) => edit.sequence)).toEqual([1, 2]);
    expect(edits.every((edit) => edit.type === 'mosaic' && edit.kind === 'screen')).toBe(true);
  });
});
