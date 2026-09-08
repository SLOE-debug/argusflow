import { expect, it } from 'vitest';
import { latestAt, mergePrivacyRanges, timelinePrivacyEdits, type PrivacyFrame } from './privacyTimeline';
import { RECORDING_FIXTURE } from './testFixtures';

it('retains a previous frame without exposing a future sample', () => {
  expect(latestAt([100, 300], 50, (time) => time)).toBeUndefined();
  expect(latestAt([100, 300], 250, (time) => time)).toBe(100);
  expect(latestAt([100, 300], 300, (time) => time)).toBe(300);
});

it('merges overlapping selections and preserves separate fragments', () => {
  expect(mergePrivacyRanges([{ start: 20, end: 40 }, { start: 0, end: 30 }, { start: 80, end: 90 }])).toEqual([{ start: 0, end: 40 }, { start: 80, end: 90 }]);
});

it('covers the frame held across a selected interval and all matching samples', () => {
  const frames: PrivacyFrame[] = [0, 100, 200].map((time, index) => ({
    sequence: index + 1, kind: 'window', shot: {
      path: `${index}.png`, captured_at_ms: time, capture_duration_ms: 0, stabilized: true,
      click_color: null, width: 800, height: 600, screen_bounds: { x: -800, y: 0, width: 800, height: 600 },
      pointer: null, crop: null, crop_failure: null,
    },
  }));
  const edits = timelinePrivacyEdits([], frames, [{ start: 50, end: 150 }], 'screenshots', 300);
  expect(edits.map((edit) => edit.sequence)).toEqual([1, 2]);
  expect(timelinePrivacyEdits([], frames, [{ start: 100, end: 100 }], 'screenshots', 300).map((edit) => edit.sequence)).toEqual([2]);
});

it('deletes only selected input events and deduplicates overlapping ranges', () => {
  const events = RECORDING_FIXTURE.trace.timeline.events;
  expect(timelinePrivacyEdits(events, [], [{ start: 0, end: 20 }, { start: 5, end: 30 }], 'inputs', 30)).toEqual([{ type: 'erase_event', sequence: 1 }]);
  expect(timelinePrivacyEdits(events, [], [{ start: 11, end: 20 }], 'everything', 30)).toEqual([]);
});
