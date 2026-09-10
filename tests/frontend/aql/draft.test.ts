import { describe, expect, it } from 'vitest';
import { DraftController } from '../../../src/features/aql/draft';
import { exportSource } from '../../../src/features/aql/files';
import type { DocumentAnalysis } from '../../../src/features/aql/contracts';

describe('中文草稿', () => {
  it('丢弃过期结果并立即撤销旧英文导出', async () => {
    const pending: ((value: DocumentAnalysis) => void)[] = [];
    const draft = new DraftController('', () => new Promise((resolve) => pending.push(resolve)));
    draft.edit('按钮()');
    pending[0]({ english: 'button()', diagnostics: [] });
    await Promise.resolve();
    expect(exportSource(draft.getSnapshot())).toBe('button()');
    draft.edit('窗口()');
    expect(exportSource(draft.getSnapshot())).toBeUndefined();
    draft.edit('窗口(');
    pending[2]({ diagnostics: [{ code: 'Syntax', message: '缺少右括号', range: { start: { line: 0, column: 3 }, end: { line: 0, column: 3 } } }] });
    pending[1]({ english: 'window()', diagnostics: [] });
    await Promise.resolve();
    expect(exportSource(draft.getSnapshot())).toBeUndefined();
    expect(draft.getSnapshot().source).toBe('窗口(');
  });
  it('输入法组合期间保留草稿并停止分析', async () => {
    let calls = 0;
    const draft = new DraftController('', async () => { calls++; return { english: 'button()', diagnostics: [] }; });
    draft.composition(true);
    draft.edit('按');
    draft.edit('按钮()');
    expect(calls).toBe(0);
    expect(exportSource(draft.getSnapshot())).toBeUndefined();
    draft.composition(false);
    await Promise.resolve();
    expect(calls).toBe(1);
    expect(exportSource(draft.getSnapshot())).toBe('button()');
  });
});
