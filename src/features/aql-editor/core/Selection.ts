/** Textarea 与编辑器状态共享的 UTF-16 selection；anchor/head 保留方向。 */
export type EditorSelection = Readonly<{ anchor: number; head: number }>;

/** 返回与方向无关的半开 selection 范围。 */
export function orderedSelection(selection: EditorSelection): readonly [number, number] {
  return selection.anchor <= selection.head
    ? [selection.anchor, selection.head]
    : [selection.head, selection.anchor];
}

/** 创建折叠光标。 */
export function caretSelection(offset: number): EditorSelection {
  return { anchor: offset, head: offset };
}
