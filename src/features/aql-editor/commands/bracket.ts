import type { EditorCommand } from '../core/Command';
import { orderedSelection } from '../core/Selection';

/** 插入括号对，并用括号包裹当前 selection。 */
export const insertParenthesisPair: EditorCommand = (text, selection) => {
  const [start, end] = orderedSelection(selection);
  const selectedText = text.slice(start, end);
  return {
    text: text.slice(0, start) + `(${selectedText})` + text.slice(end),
    selection: selectedText.length > 0
      ? { anchor: start + 1, head: end + 1 }
      : { anchor: start + 1, head: start + 1 },
  };
};

/** 光标位于自动补齐的右括号前时直接越过它。 */
export const skipClosingParenthesis: EditorCommand = (text, selection) => {
  if (selection.anchor === selection.head && text[selection.head] === ')') {
    const nextOffset = selection.head + 1;
    return { text, selection: { anchor: nextOffset, head: nextOffset } };
  }
  const [start, end] = orderedSelection(selection);
  return {
    text: text.slice(0, start) + ')' + text.slice(end),
    selection: { anchor: start + 1, head: start + 1 },
  };
};
