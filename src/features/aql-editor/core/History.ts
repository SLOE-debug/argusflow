import type { EditorSelection } from './Selection';

/** Undo/redo 同时恢复的文档与 selection 快照。 */
export type HistorySnapshot = Readonly<{ text: string; selection: EditorSelection }>;

/** 自研编辑器的线性 undo/redo 历史，不依赖浏览器 DOM undo 栈。 */
export class EditorHistory {
  #past: HistorySnapshot[] = [];
  #future: HistorySnapshot[] = [];

  /** 记录一次命令执行前的状态，并清空 redo 分支。 */
  push(snapshot: HistorySnapshot): void {
    const previous = this.#past.at(-1);
    if (previous?.text === snapshot.text && previous.selection.head === snapshot.selection.head) {
      return;
    }
    this.#past.push(snapshot);
    this.#future = [];
  }

  /** 返回上一快照并把当前状态放入 redo 栈。 */
  undo(current: HistorySnapshot): HistorySnapshot | null {
    const previous = this.#past.pop() ?? null;
    if (previous) {
      this.#future.push(current);
    }
    return previous;
  }

  /** 返回下一快照并把当前状态放回 undo 栈。 */
  redo(current: HistorySnapshot): HistorySnapshot | null {
    const next = this.#future.pop() ?? null;
    if (next) {
      this.#past.push(current);
    }
    return next;
  }
}
