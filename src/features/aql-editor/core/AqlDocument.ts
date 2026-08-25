import type { TextEdit } from '../language/types';
import { applyTextEdits } from './TextEdit';

/** 自研编辑器的不可变文本模型。 */
export class AqlDocument {
  readonly text: string;

  /** 从完整文本创建不可变文档。 */
  constructor(text: string) {
    this.text = text;
  }

  /** 应用一组 Rust language service edit 并返回新文档。 */
  apply(edits: readonly TextEdit[]): AqlDocument {
    return new AqlDocument(applyTextEdits(this.text, edits));
  }

  /** 替换 JavaScript UTF-16 offset 范围。 */
  replace(start: number, end: number, newText: string): AqlDocument {
    return new AqlDocument(this.text.slice(0, start) + newText + this.text.slice(end));
  }
}
