/** AQL backend compiler 使用的稳定后端家族。 */
export type QueryBackend = 'windows_uia' | 'browser_cdp' | 'vision';

/** Runtime Planner 可选择的实际执行后端。 */
export type BackendKind =
  | 'windows_uia'
  | 'browser_cdp'
  | 'ocr_small'
  | 'send_input';

/** 语言服务和 backend compiler 共享的稳定诊断代码。 */
export type AqlDiagnosticCode =
  | 'empty_query'
  | 'invalid_token'
  | 'unexpected_token'
  | 'unknown_role'
  | 'unknown_property'
  | 'unknown_operator'
  | 'invalid_predicate'
  | 'invalid_regex'
  | 'invalid_argument'
  | 'css_syntax'
  | 'missing_right_parenthesis'
  | 'unexpected_right_parenthesis'
  | 'backend_specific_property'
  | 'residual_filter'
  | 'expensive_traversal'
  | 'potential_multi_match'
  | 'unsupported_backend'
  | 'runtime_unavailable';

/** AQL 诊断的稳定严重级别。 */
export type AqlDiagnosticSeverity = 'error' | 'warning' | 'information';

/** WebView 文本协议位置，列按 UTF-16 code unit 且从零开始。 */
export type EditorPosition = { line: number; utf16_column: number };

/** WebView 文本协议半开范围。 */
export type EditorRange = { start: EditorPosition; end: EditorPosition };

/** 诊断本地化所需的结构化参数。 */
export type AqlDiagnosticParams =
  | { type: 'none' }
  | { type: 'token'; token: string }
  | { type: 'expected'; expected: string }
  | { type: 'minimum_count'; minimum: number };

/** Rust domain 不携带产品文案的结构化诊断。 */
export type AqlDiagnostic = {
  code: AqlDiagnosticCode;
  severity: AqlDiagnosticSeverity;
  range: EditorRange | null;
  backend: QueryBackend | null;
  params: AqlDiagnosticParams;
};
