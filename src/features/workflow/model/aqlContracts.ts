/** AQL backend compiler 使用的稳定后端家族。 */
export type QueryBackend = 'windows_uia' | 'browser_cdp' | 'vision';

/** Runtime Planner 可选择的实际执行后端。 */
export type BackendKind =
  | 'windows_uia'
  | 'browser_cdp'
  | 'ocr_small'
  | 'send_input';

/** 后端保持 AQL 语义所需的执行方式。 */
export type QuerySupportLevel = 'native' | 'hybrid' | 'emulated' | 'unsupported';

/** AQL 查询计划的粗粒度预计成本。 */
export type QueryCost = 'low' | 'medium' | 'high';

/** AQL 是否只依赖跨平台语义。 */
export type QueryPortability =
  | { type: 'portable' }
  | { type: 'backend_specific'; backends: readonly QueryBackend[] };

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

/** Executor 实现与当前运行环境是否允许实际执行。 */
export type RuntimeAvailability =
  | 'ready' | 'missing_context' | 'unavailable' | 'not_implemented';

/** 后端与当前前台窗口、进程或浏览器会话的匹配程度。 */
export type ContextFitness = 'excellent' | 'good' | 'neutral' | 'poor';

/** Prepared backend plan 的开发者 Explain 步骤类别。 */
export type PlanStepKind =
  | 'scope' | 'candidate_source' | 'pushdown' | 'cache'
  | 'residual' | 'selection' | 'traversal' | 'action';

/** 单个真实 backend prepared candidate 的只读 Explain。 */
export type PlanExplain = {
  /** 实际 backend 类别。 */
  backend: BackendKind;
  /** 候选唯一对应的完整 any fallback 路径；拒绝候选为 null。 */
  branch_path: readonly number[] | null;
  /** Compiler 从真实逻辑计划推导的语义支持。 */
  support: QuerySupportLevel;
  /** Compiler 估算成本。 */
  cost: QueryCost;
  /** Executor 与当前上下文状态。 */
  availability: RuntimeAvailability;
  /** 当前上下文适配度。 */
  context_fitness: ContextFitness;
  /** 查询可移植性。 */
  portability: QueryPortability;
  /** 真实 prepared plan 的开发者步骤。 */
  steps: readonly { kind: PlanStepKind; summary: string }[];
  /** Compiler 产生的结构化诊断。 */
  diagnostics: readonly AqlDiagnostic[];
};

/** Runtime Planner 排序后的候选集合与实际选择。 */
export type PlanningReport = {
  /** 没有 Ready 候选时为 null。 */
  selected_backend: BackendKind | null;
  /** 按 Planner 规则排序的候选。 */
  candidates: readonly PlanExplain[];
};

/** `inspect_aql` 返回的 Runtime Planner Explain 与恢复诊断。 */
export type AqlInspection =
  | {
      status: 'valid';
      canonical_source: string;
      portability: QueryPortability;
      diagnostics: readonly AqlDiagnostic[];
      planning: PlanningReport;
    }
  | { status: 'invalid'; diagnostics: readonly AqlDiagnostic[] };
