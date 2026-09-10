import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { EditorWorkspace } from '../../../src/components/editor/EditorWorkspace';
import type { LanguageService } from '../../../src/features/aql/contracts';

vi.mock('../../../src/components/editor/monaco/CodeEditor', () => ({
  CodeEditor: ({ source, onChange, onComposition }: { source: string; onChange: (s: string) => void; onComposition: (active: boolean) => void }) => (
    <textarea aria-label="中文查询" value={source} onChange={(e) => onChange(e.target.value)} onCompositionStart={() => onComposition(true)} onCompositionEnd={() => onComposition(false)} />
  ),
}));
const service: LanguageService = {
  inspect: (source) => source.endsWith(')') ? { english: 'button()', formatted: '按钮()', diagnostics: [] } :
    { diagnostics: [{ code: 'Syntax', message: '需要右括号', range: { start: { line: 0, column: 3 }, end: { line: 0, column: 3 } } }] },
  completions: () => [], hover: () => undefined, importEnglish: () => '按钮()',
};
it('只允许当前合法草稿导出并在组合输入时隐藏旧预览', async () => {
  render(<EditorWorkspace service={service} />);
  const exportButton = screen.getByRole('button', { name: '导出英文 AQL' });
  await waitFor(() => expect(exportButton).toBeEnabled());
  const editor = screen.getByLabelText('中文查询');
  fireEvent.compositionStart(editor);
  fireEvent.change(editor, { target: { value: '按钮(' } });
  expect(exportButton).toBeDisabled();
  expect(screen.getByLabelText('英文 AQL 预览')).not.toHaveTextContent('button()');
  fireEvent.compositionEnd(editor);
  await screen.findByText(/需要右括号/);
  expect(exportButton).toBeDisabled();
});
it('格式化回写来自语言服务的中文源码', async () => {
  render(<EditorWorkspace service={service} />);
  const button = screen.getByRole('button', { name: '格式化' });
  await waitFor(() => expect(button).toBeEnabled());
  fireEvent.click(button);
  expect(screen.getByLabelText('中文查询')).toHaveValue('按钮()');
});

it('迟到的文件导入不会覆盖导入期间的新输入', async () => {
  let finish!: (buffer: ArrayBuffer) => void;
  const file = new File([], 'query.aql');
  Object.defineProperty(file, 'arrayBuffer', { value: () => new Promise<ArrayBuffer>((resolve) => { finish = resolve; }) });
  render(<EditorWorkspace service={service} />);
  fireEvent.change(screen.getByLabelText('选择 AQL 文件'), { target: { files: [file] } });
  fireEvent.change(screen.getByLabelText('中文查询'), { target: { value: '输入框()' } });
  await act(async () => { finish(new TextEncoder().encode('button()').buffer); });
  expect(screen.getByLabelText('中文查询')).toHaveValue('输入框()');
});

it('有效导入经语言服务回写并允许导出英文', async () => {
  const file = new File([], 'query.aql');
  Object.defineProperty(file, 'arrayBuffer', { value: async () => new TextEncoder().encode('button()').buffer });
  render(<EditorWorkspace service={service} />);
  fireEvent.change(screen.getByLabelText('选择 AQL 文件'), { target: { files: [file] } });
  await waitFor(() => expect(screen.getByLabelText('中文查询')).toHaveValue('按钮()'));
  expect(screen.getByRole('button', { name: '导出英文 AQL' })).toBeEnabled();
  expect(screen.getByLabelText('英文 AQL 预览')).toHaveTextContent('button()');
});
