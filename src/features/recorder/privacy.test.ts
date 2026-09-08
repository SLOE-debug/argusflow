import { expect, it } from 'vitest';
import { ocrPrivacyRect, privacyKeywords, recordingSearchText } from './privacy';
import { RECORDING_FIXTURE } from './testFixtures';
import type { RawTraceEvent } from './model';

it('clamps OCR polygons to the original screenshot and rejects invalid geometry', () => {
  expect(ocrPrivacyRect({ raw_text: 'private', confidence: 1, polygon: [{ x: -1, y: 3 }, { x: 99, y: 20 }] }, 40, 12))
    .toEqual({ x: 0, y: 1, width: 40, height: 11 });
  expect(ocrPrivacyRect({ raw_text: '', confidence: 1, polygon: [{ x: NaN, y: 0 }] }, 40, 12)).toBeNull();
  expect(privacyKeywords(' Alice，1380000\n公司 ')).toEqual(['alice', '1380000', '公司']);
});

it('finds words across individual key records including key releases', () => {
  const source = RECORDING_FIXTURE.trace.timeline.events[0];
  const events: RawTraceEvent[] = ['a', 'b'].flatMap((value, index) => (['down', 'up'] as const).map((phase, offset) => ({
    ...source, sequence: index * 2 + offset, elapsed_ms: index * 20 + offset,
    input: { type: 'key', phase, virtual_key: 65, scan_code: 1, flags: 0, chord: null, text: { type: 'plain', value } },
  })));
  expect([...recordingSearchText(events).values()].every((text) => text.includes('ab'))).toBe(true);
});
