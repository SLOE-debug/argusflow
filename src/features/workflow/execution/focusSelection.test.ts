import { describe, expect, it } from 'vitest';

import { deriveFocusSelection, type FocusCandidate } from './focusSelection';

const candidate = (id: string, confidence: number): FocusCandidate => ({
  id,
  rawText: id,
  confidence,
  polygon: [],
  bbox: { x: 10, y: 10, width: 30, height: 20 },
});

describe('deriveFocusSelection', () => {
  it('preserves strict 0/1/N semantics instead of selecting highest confidence', () => {
    expect(deriveFocusSelection([]).outcome).toBe('not_found');
    expect(deriveFocusSelection([candidate('only', 0.81)]).outcome).toBe('unique');

    const ambiguous = deriveFocusSelection([
      candidate('first-ranked', 0.82),
      candidate('higher-confidence', 0.99),
    ]);
    expect(ambiguous.outcome).toBe('ambiguous');
    expect(ambiguous.candidates.map((item) => item.id)).toEqual([
      'first-ranked',
      'higher-confidence',
    ]);
  });
});
