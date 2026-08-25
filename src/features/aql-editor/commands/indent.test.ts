import { describe, expect, it } from 'vitest';

import { indentSelection, insertIndentedLine } from './indent';

describe('AQL indent commands', () => {
  it('adds one level after an opening parenthesis', () => {
    const result = insertIndentedLine('button(', { anchor: 7, head: 7 });

    expect(result).toEqual({
      text: 'button(\n    ',
      selection: { anchor: 12, head: 12 },
    });
  });

  it('indents every selected line', () => {
    const result = indentSelection('name = "保存"\nenabled = true', {
      anchor: 0,
      head: 26,
    });

    expect(result.text).toBe('    name = "保存"\n    enabled = true');
  });
});
