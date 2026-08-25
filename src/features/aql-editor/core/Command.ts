import type { EditorSelection } from './Selection';

/** 一个编辑命令的原子结果。 */
export type CommandResult = Readonly<{ text: string; selection: EditorSelection }>;

/** 与 React view 解耦的编辑命令。 */
export type EditorCommand = (
  text: string,
  selection: EditorSelection,
) => CommandResult;
