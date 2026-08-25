/** 可在前后端无损传递的 JSON 值。 */
export type JsonValue = null | boolean | number | string | JsonValue[] | { [key: string]: JsonValue };
export type JsonObject = { [key: string]: JsonValue };

/** 与 Rust 后端交换的 schema v4 工作流。 */
export type WorkflowDefinition = {
  /** 当前契约固定版本。 */
  schema_version: 4;
  /** 工作流稳定 ID。 */
  id: string;
  /** 面向用户的名称。 */
  name: string;
  /** Condition 读取的只读 JSON 对象。 */
  variables: JsonObject;
  /** 对进程和 shell 能力的显式授权。 */
  permissions: WorkflowPermissions;
  /** 可执行节点。 */
  nodes: WorkflowNodeContract[];
  /** 节点间有向连接。 */
  edges: WorkflowEdgeContract[];
};

export type Position = { x: number; y: number };

/** 后端可执行节点的通用字段与具体节点类型联合。 */
export type WorkflowNodeContract = { id: string; position: Position } & WorkflowNodeKind;

export type ConditionOperator =
  | 'equal' | 'not_equal' | 'greater_than' | 'greater_than_or_equal'
  | 'less_than' | 'less_than_or_equal' | 'contains' | 'exists'
  | 'not_exists' | 'is_empty' | 'not_empty';

/** 使用 JSON Pointer 读取变量的结构化安全条件。 */
export type ConditionPredicate = {
  /** RFC 6901 JSON Pointer。 */
  pointer: string;
  /** 安全结构化运算符。 */
  operator: ConditionOperator;
  /** 二元运算符的 JSON 右值。 */
  operand: JsonValue | null;
};

export type WorkflowNodeKind =
  | { type: 'start' }
  | { type: 'log'; message: string }
  | { type: 'debug'; value: ValueExpr }
  | { type: 'delay'; milliseconds: number }
  | { type: 'condition'; predicate: ConditionPredicate }
  | { type: 'application'; spec: ApplicationSpec }
  | { type: 'ui'; operation: UiOperation }
  | { type: 'command'; operation: CommandOperation }
  | { type: 'end' };

/** Workflow 层保存的语义界面操作。 */
export type UiOperation =
  | { type: 'click'; target: AutomationTarget }
  | { type: 'set_value'; target: AutomationTarget; value: ValueExpr }
  | { type: 'get_text'; target: AutomationTarget }
  | { type: 'get_value'; target: AutomationTarget };

/** UI 节点允许选择的强类型操作类别。 */
export type UiOperationKind = UiOperation['type'];

/** AQL 语义与执行后端选择分离的动作目标。 */
export type AutomationTarget = {
  /** 当前上下文或显式应用会话作用域。 */
  scope: TargetScope;
  /** 跨平台定位契约。 */
  locator: TargetLocator;
  /** `auto` 默认根据查询能力规划，另外两项用于显式强制后端。 */
  backend_preference: BackendPreference;
};

/** 资源引用与普通 JSON 值引用保持独立。 */
export type ResourceRef = {
  producer_node_id: string;
  output_name: string;
};

/** UI 操作使用的逻辑资源作用域。 */
export type TargetScope =
  | { type: 'current' }
  | { type: 'application'; resource: ResourceRef };

/** 节点参数的数据来源表达式。 */
export type ValueExpr =
  | { type: 'literal'; value: JsonValue }
  | { type: 'workflow_input'; key: string }
  | { type: 'node_output'; node_id: string; output: string }
  | { type: 'variable'; name: string };

export type ValueExprKind = ValueExpr['type'];

/** 查询规划时独立于 AQL 语义的后端偏好。 */
export type BackendPreference = 'auto' | 'windows_uia' | 'browser_cdp';

/** 与 workflow schema 独立演进的持久化 AQL 源码。 */
export type AqlQuery = {
  language_version: 1;
  source: string;
};

/** AQL、显式视觉查询或物理坐标组成的目标判别联合。 */
export type TargetLocator =
  | { type: 'query'; query: AqlQuery }
  | { type: 'visual'; query: { text: string; exact: boolean } }
  | { type: 'coordinate'; point: { x: number; y: number } };

/** 应用资源节点获取 direct-process Windows 桌面应用的契约。 */
export type ApplicationSpec = {
  /** 用于进程身份匹配和启动的绝对 EXE 路径。 */
  executable_path: string;
  /** 不经过 shell 解析、直接传给 EXE 的参数。 */
  arguments: string[];
  /** 唯一顶层窗口的标题匹配规则。 */
  window_title: WindowTitleMatcher;
  /** 复用或启动进程的策略。 */
  acquire_policy: AcquirePolicy;
  /** 启动后等待顶层窗口的最长毫秒数。 */
  launch_timeout_ms: number;
  /** 工作流结束后的应用清理策略。 */
  cleanup_policy: CleanupPolicy;
  /** 获取时的窗口激活要求。 */
  activation_policy: ActivationPolicy;
};

export type AcquirePolicy = 'attach_or_start' | 'attach_only' | 'always_start_new';
export type CleanupPolicy = 'leave_running' | 'close_if_started_by_workflow' | 'always_close';
export type ActivationPolicy = 'none' | 'best_effort' | 'required';

/** Windows 顶层窗口标题支持精确或包含匹配。 */
export type WindowTitleMatcher =
  | { type: 'equal'; value: string }
  | { type: 'contains'; value: string };

/** Action 属性面板允许切换的目标定位类别。 */
export type TargetLocatorKind = TargetLocator['type'];

/** Command 节点的三种同语义运行器。 */
export type CommandRunner = 'direct' | 'power_shell' | 'cmd';

export type EnvironmentBinding = {
  name: string;
  value: ValueExpr;
};

/** 独立于 UI Planner 的命令执行契约。 */
export type CommandOperation = {
  runner: CommandRunner;
  program: ValueExpr | null;
  arguments: ValueExpr[];
  script: ValueExpr | null;
  working_directory: ValueExpr | null;
  environment: EnvironmentBinding[];
  stdin: ValueExpr | null;
  timeout_ms: number;
  accepted_exit_codes: number[];
  max_stdout_bytes: number;
  max_stderr_bytes: number;
};

/** 工作流对命令能力的显式授权。 */
export type WorkflowPermissions = {
  process_spawn: boolean;
  powershell: boolean;
  cmd: boolean;
};

/** AQL backend compiler 使用的稳定后端家族。 */
export type QueryBackend = 'windows_uia' | 'browser_cdp' | 'vision';

/** Runtime Planner 可选择的实际执行后端。 */
export type BackendKind =
  | 'windows_uia'
  | 'browser_cdp'
  | 'visual_cache'
  | 'ocr_tiny'
  | 'ocr_medium'
  | 'gui_grounding'
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

export type ConditionBranch = 'true' | 'false';

/** 有向边；只有 Condition 源节点携带 branch。 */
export type WorkflowEdgeContract = {
  /** 文档内唯一 ID。 */
  id: string;
  /** 源节点 ID。 */
  source: string;
  /** 目标节点 ID。 */
  target: string;
  /** Condition 源节点的分支。 */
  branch: ConditionBranch | null;
};

/** Rust `ValidationIssueCode` 的完整序列化取值。 */
export type ValidationIssueCode =
  | 'unsupported_schema_version'
  | 'empty_workflow_name'
  | 'invalid_variables'
  | 'empty_node_id'
  | 'duplicate_node_id'
  | 'empty_edge_id'
  | 'duplicate_edge_id'
  | 'invalid_start_count'
  | 'invalid_end_count'
  | 'unknown_edge_endpoint'
  | 'self_loop'
  | 'invalid_node_degree'
  | 'invalid_condition'
  | 'invalid_branch'
  | 'cycle_detected'
  | 'unreachable_node'
  | 'no_path_to_end'
  | 'empty_log_message'
  | 'invalid_delay'
  | 'invalid_aql_query'
  | 'invalid_application_spec'
  | 'invalid_command'
  | 'command_permission_denied'
  | 'invalid_value_reference'
  | 'invalid_resource_reference'
  | 'reference_not_dominating';

export type ValidationIssue = {
  code: ValidationIssueCode;
  message: string;
  node_id: string | null;
  edge_id: string | null;
};

export type ValidationReport = {
  valid: boolean;
  issues: readonly ValidationIssue[];
};
export type RunStarted = { run_id: string };

export type ExecutionEventKind =
  | 'workflow_started' | 'node_started' | 'log' | 'node_output_produced'
  | 'resource_acquired' | 'backend_selected' | 'command_exited' | 'node_succeeded'
  | 'edge_traversed' | 'node_failed' | 'workflow_completed' | 'workflow_failed';

export type ExecutionEvent = {
  /** 运行实例 ID。 */
  run_id: string;
  /** 工作流 ID。 */
  workflow_id: string;
  /** 运行内严格递增序号。 */
  sequence: number;
  /** 相关节点 ID。 */
  node_id: string | null;
  /** 相关连线 ID。 */
  edge_id: string | null;
  /** 生命周期类别。 */
  kind: ExecutionEventKind;
  /** 可选运行说明。 */
  message: string | null;
  /** 不包含业务输出或 OS handle 的结构化载荷。 */
  payload: ExecutionEventPayload | null;
};

export type ExecutionEventPayload =
  | { type: 'node_outputs_produced'; output_names: string[] }
  | { type: 'resource_acquired'; output_name: string; resource_type: string }
  | { type: 'backend_selected'; backend: BackendKind }
  | { type: 'command_exited'; exit_code: number };

/** Rust `CommandErrorCode` 的完整序列化取值。 */
export const COMMAND_ERROR_CODES = [
  'validation_failed',
  'run_in_progress',
  'event_delivery_failed',
  'execution_invariant_failed',
  'automation_failed',
  'application_failed',
  'command_failed',
  'runtime_data_failed',
] as const;

export type BackendCommandErrorCode = typeof COMMAND_ERROR_CODES[number];
export type CommandErrorCode = BackendCommandErrorCode | 'unknown_error';

export type CommandError = {
  code: CommandErrorCode;
  message: string;
  issues: readonly ValidationIssue[];
};
