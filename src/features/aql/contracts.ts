/** 零基行及 UTF-16 列，与 Rust WASM 协议一致。 */
export interface Position { readonly line: number; readonly column: number }
/** 半开文本范围。 */
export interface Range { readonly start: Position; readonly end: Position }
/** 当前草稿诊断。 */
export interface Diagnostic {
  readonly code: string;
  readonly message: string;
  readonly range: Range;
}
/** 同一次分析生成的结果，英文源码只在语法有效时存在。 */
export interface DocumentAnalysis {
  readonly english?: string;
  readonly formatted?: string;
  readonly diagnostics: readonly Diagnostic[];
}
/** AQL 符号类别。 */
export type SymbolKind = 'Role' | 'Function' | 'Attribute' | 'Operator' | 'Boolean' | 'Parameter';
/** 编辑器补全及其中文文本范围。 */
export interface Completion {
  readonly label: string;
  readonly filter_text: string;
  readonly insert_text: string;
  readonly insert_as_snippet: boolean;
  readonly detail: string;
  readonly description: string;
  readonly example: string;
  readonly range: Range;
  readonly kind: SymbolKind;
}
/** 中英文词汇对照说明。 */
export interface Hover {
  readonly title: string;
  readonly kind: SymbolKind;
  readonly signature: string;
  readonly description: string;
  readonly example: string;
  readonly range: Range;
}
/** 唯一语言服务，视图不自行解析查询。 */
export interface LanguageService {
  inspect(source: string): DocumentAnalysis;
  completions(source: string, position: Position): readonly Completion[];
  hover(source: string, position: Position): Hover | undefined;
  importEnglish(source: string): string;
}
/** 草稿状态携带版本，过期异步结果不能覆盖当前输入。 */
export type DraftState =
  | { readonly status: 'pending'; readonly source: string; readonly revision: number }
  | { readonly status: 'ready'; readonly source: string; readonly revision: number; readonly analysis: DocumentAnalysis }
  | { readonly status: 'failed'; readonly source: string; readonly revision: number; readonly message: string };
