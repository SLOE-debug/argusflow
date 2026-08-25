import { describe, expect, it } from 'vitest';

import { EditorHistory } from './History';

describe('EditorHistory', () => {
  it('supports undo and redo without relying on DOM history', () => {
    const history = new EditorHistory();
    history.push({ text: 'button', selection: { anchor: 6, head: 6 } });

    const undone = history.undo({ text: 'button()', selection: { anchor: 7, head: 7 } });
    expect(undone?.text).toBe('button');

    const redone = history.redo(undone!);
    expect(redone?.text).toBe('button()');
  });
});
