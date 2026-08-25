import type { EditorCommand } from '../core/Command';
import { orderedSelection } from '../core/Selection';

const INDENT = '    ';

/** 使用当前行缩进，并在左括号后增加一级缩进。 */
export const insertIndentedLine: EditorCommand = (text, selection) => {
  const [start, end] = orderedSelection(selection);
  const lineStart = text.lastIndexOf('\n', start - 1) + 1;
  const currentIndent = text.slice(lineStart, start).match(/^\s*/u)?.[0] ?? '';
  const extraIndent = text.slice(0, start).trimEnd().endsWith('(') ? INDENT : '';
  const insertion = `\n${currentIndent}${extraIndent}`;
  const nextOffset = start + insertion.length;
  return {
    text: text.slice(0, start) + insertion + text.slice(end),
    selection: { anchor: nextOffset, head: nextOffset },
  };
};

/** 对 selection 覆盖的每一行增加四空格缩进。 */
export const indentSelection: EditorCommand = (text, selection) => {
  const [start, end] = orderedSelection(selection);
  const blockStart = text.lastIndexOf('\n', Math.max(0, start - 1)) + 1;
  const selectedBlock = text.slice(blockStart, end);
  const indented = selectedBlock.replace(/^/gmu, INDENT);
  const lineCount = selectedBlock.split('\n').length;
  return {
    text: text.slice(0, blockStart) + indented + text.slice(end),
    selection: {
      anchor: start + INDENT.length,
      head: end + (INDENT.length * lineCount),
    },
  };
};
