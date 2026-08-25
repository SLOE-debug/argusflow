import { describe, expect, it } from 'vitest';

import { LineIndex } from './LineIndex';

describe('LineIndex', () => {
  it('uses UTF-16 offsets for Chinese and emoji positions', () => {
    const text = 'button(name = "保存😀")\ntext()';
    const index = new LineIndex(text);
    const emojiOffset = text.indexOf('😀');

    expect(index.toPosition(emojiOffset)).toEqual({ line: 0, utf16_column: 17 });
    expect(index.toOffset({ line: 0, utf16_column: 19 })).toBe(emojiOffset + 2);
    expect(index.toPosition(text.indexOf('text'))).toEqual({ line: 1, utf16_column: 0 });
  });
});
