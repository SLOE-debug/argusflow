import type { EditorPosition, EditorRange } from '../../workflow/contracts';

/** 以 JavaScript 原生 UTF-16 offset 建立行索引。 */
export class LineIndex {
  readonly #text: string;
  readonly #lineStarts: readonly number[];

  constructor(text: string) {
    this.#text = text;
    const lineStarts = [0];
    for (let offset = 0; offset < text.length; offset += 1) {
      if (text[offset] === '\n') {
        lineStarts.push(offset + 1);
      }
    }
    this.#lineStarts = lineStarts;
  }

  /** 将 Rust 协议行列转换为 textarea/String.slice 使用的 UTF-16 offset。 */
  toOffset(position: EditorPosition): number {
    const lineStart = this.#lineStarts[position.line] ?? this.#text.length;
    const nextLineStart = this.#lineStarts[position.line + 1] ?? this.#text.length;
    return Math.min(lineStart + position.utf16_column, nextLineStart);
  }

  /** 将 textarea UTF-16 offset 转换为 Rust 协议位置。 */
  toPosition(rawOffset: number): EditorPosition {
    const offset = Math.max(0, Math.min(rawOffset, this.#text.length));
    let low = 0;
    let high = this.#lineStarts.length;
    while (low + 1 < high) {
      const middle = Math.floor((low + high) / 2);
      if ((this.#lineStarts[middle] ?? 0) <= offset) {
        low = middle;
      } else {
        high = middle;
      }
    }
    return { line: low, utf16_column: offset - (this.#lineStarts[low] ?? 0) };
  }

  /** 将协议范围转换为 JavaScript 半开 offset 对。 */
  toOffsets(range: EditorRange): readonly [number, number] {
    return [this.toOffset(range.start), this.toOffset(range.end)];
  }

  /** 返回包括空文档第一行在内的总行数。 */
  get lineCount(): number {
    return this.#lineStarts.length;
  }
}
