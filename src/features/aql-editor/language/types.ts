import type {
  AqlDiagnostic,
  EditorPosition,
  EditorRange,
} from '../../workflow/contracts';

/** Rust AQL language service 返回的语法高亮类别。 */
export type SyntaxTokenKind =
  | 'role' | 'function' | 'property' | 'namespace' | 'operator'
  | 'string' | 'regex' | 'boolean' | 'integer' | 'punctuation'
  | 'trivia' | 'unknown';

/** Rust semantic token 及其 UTF-16 范围。 */
export type SyntaxToken = {
  /** Rust language service 判定的类别。 */
  kind: SyntaxTokenKind;
  /** 浏览器安全范围。 */
  range: EditorRange;
};

/** 补全候选类别。 */
export type CompletionItemKind = 'role' | 'function' | 'property' | 'operator' | 'value';

/** Rust grammar 生成的补全候选。 */
export type CompletionItem = {
  /** 补全列表标签。 */
  label: string;
  /** 应被替换的源码范围。 */
  replacement_range: EditorRange;
  /** 应写入文档的文本。 */
  insert_text: string;
  /** 候选类别。 */
  kind: CompletionItemKind;
  /** 可选语言说明。 */
  detail: string | null;
};

/** Rust language service 返回的 token Hover。 */
export type Hover = {
  /** 被说明的 token 范围。 */
  range: EditorRange;
  /** 稳定符号名。 */
  symbol: string;
  /** 供产品层本地化的说明代码。 */
  description_code: string;
};

/** 可以原子应用到编辑器文档的 UTF-16 文本修改。 */
export type TextEdit = { range: EditorRange; new_text: string };

/** Editor 实际消费的 WASM 文档投影，不复制 Rust HIR。 */
export type LanguageDocument = {
  parsed: {
    diagnostics: readonly AqlDiagnostic[];
    semantic_tokens: readonly SyntaxToken[];
    hir: unknown | null;
  };
  formatted_source: string | null;
  canonical_source: string | null;
};

/** 自研编辑器依赖的同源 Rust/WASM 语言能力。 */
export type AqlLanguageService = Readonly<{
  inspect: (source: string) => LanguageDocument;
  completions: (source: string, position: EditorPosition) => readonly CompletionItem[];
  hover: (source: string, position: EditorPosition) => Hover | null;
  bracketPair: (source: string, position: EditorPosition) => readonly EditorRange[] | null;
  codeActions: (source: string) => readonly TextEdit[];
}>;
