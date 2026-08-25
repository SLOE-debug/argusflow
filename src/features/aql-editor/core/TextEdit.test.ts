import { describe, expect, it } from 'vitest';

import { applyTextEdits } from './TextEdit';

describe('applyTextEdits', () => {
  it('applies UTF-16 edits around an emoji without shifting later ranges', () => {
    const source = 'button(name = "😀")[enabled=true]';
    const result = applyTextEdits(source, [
      {
        range: {
          start: { line: 0, utf16_column: 19 },
          end: { line: 0, utf16_column: 20 },
        },
        new_text: '(',
      },
      {
        range: {
          start: { line: 0, utf16_column: 32 },
          end: { line: 0, utf16_column: 33 },
        },
        new_text: ')',
      },
    ]);

    expect(result).toBe('button(name = "😀")(enabled=true)');
  });
});
