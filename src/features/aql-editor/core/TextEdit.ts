import type { EditorRange } from '../../workflow/contracts';
import type { TextEdit } from '../language/types';
import { LineIndex } from './LineIndex';

/** JavaScript String.slice 可以直接消费的 UTF-16 offset edit。 */
export type OffsetTextEdit = Readonly<{ start: number; end: number; newText: string }>;

/** 将 UTF-16 行列协议 edit 转换为 JavaScript offset edit。 */
export function toOffsetEdit(text: string, edit: TextEdit): OffsetTextEdit {
  const index = new LineIndex(text);
  const [start, end] = index.toOffsets(edit.range);
  return { start, end, newText: edit.new_text };
}

/** 从后向前原子应用多项 edit，避免前项改变后项 offset。 */
export function applyTextEdits(text: string, edits: readonly TextEdit[]): string {
  const offsetEdits = edits
    .map((edit) => toOffsetEdit(text, edit))
    .sort((left, right) => right.start - left.start);
  return offsetEdits.reduce(
    (current, edit) => current.slice(0, edit.start) + edit.newText + current.slice(edit.end),
    text,
  );
}

/** 构造覆盖整个文档的协议范围。 */
export function documentRange(text: string): EditorRange {
  const index = new LineIndex(text);
  return {
    start: { line: 0, utf16_column: 0 },
    end: index.toPosition(text.length),
  };
}
