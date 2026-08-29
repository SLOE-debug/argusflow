import type { ExecutionComponentFrame } from '../components/reusableFlowContracts';
import type { KeyChord } from './inputContracts';
import type { VisualQueryExpr } from './visual';
export type {
  TargetWaitMode,
  TargetWaitPolicy,
  UiExecutionPolicy,
  UiPostcondition,
} from './uiExecutionContracts';
/** 可在前后端无损传递的 JSON 值。 */
export type JsonValue = null | boolean | number | string | JsonValue[] | { [key: string]: JsonValue };
export type JsonObject = { [key: string]: JsonValue };
/** 与 Rust 后端交换的 schema v8 工作流。 */
export type WorkflowDefinition = {
  /** 当前契约固定版本。 */
  schema_version: 8;
  /** 工作流稳定 ID。 */
  id: string;
  /** 面向用户的名称。 */
  name: string;
  /** 瞬时运行输入的持久化声明，不包含实际值。 */
  inputs: WorkflowInputDefinition[];
  /** 每次运行复制后可由变量节点事务式更新的初始 JSON 对象。 */
  variables: JsonObject;
  /** 对进程和 shell 能力的显式授权。 */
  permissions: WorkflowPermissions;
  /** 可执行节点。 */
  nodes: WorkflowNodeContract[];
  /** 节点间有向连接。 */
  edges: WorkflowEdgeContract[];
};
export type Position = { x: number; y: number };
/** 后端可执行节点的通用字段与开放 definition envelope。 */
export type WorkflowNodeContract = {
  id: string;
  position: Position;
  /** 指向唯一 NodeCompiler 的稳定类型 ID。 */
  type_id: string;
  /** 节点类型独立于工作流 schema 的 payload 版本。 */
  version: number;
  /** 只由对应注册编译器解码的节点参数。 */
  payload: JsonValue;
  /** 在原生结果冻结快照上计算并原子合并的公开输出。 */
  output_bindings: Readonly<Record<string, ValueExpr>>;
};

export type ConditionOperator =
  | 'equal' | 'not_equal' | 'greater_than' | 'greater_than_or_equal'
  | 'less_than' | 'less_than_or_equal' | 'contains' | 'exists'
  | 'not_exists' | 'is_empty' | 'not_empty';

/** Workflow 层保存的语义界面操作。 */
export type UiOperation =
  | { type: 'click'; target: AutomationTarget }
  | { type: 'set_value'; target: AutomationTarget; value: ValueExpr }
  | { type: 'press_key'; target: AutomationTarget; chord: KeyChord }
  | { type: 'type_text'; target: AutomationTarget; value: ValueExpr }
  | { type: 'get_text'; target: AutomationTarget }
  | { type: 'get_value'; target: AutomationTarget }
  | {
      type: 'extract';
      target: AutomationTarget;
      cardinality: ExtractCardinality;
      fields: FieldProjection[];
    }
  | { type: 'collect_links'; target: AutomationTarget };

/** Extract 操作返回唯一对象还是对象数组。 */
export type ExtractCardinality = 'one' | 'many';

/** Extract 字段读取来源。 */
export type FieldProjectionSource =
  | { type: 'text' }
  | { type: 'value' }
  | { type: 'name' }
  | { type: 'property'; name: string }
  | { type: 'attribute'; name: string };

/** Extract 输出对象中的一个具名字段。 */
export type FieldProjection = Readonly<{
  name: string;
  source: FieldProjectionSource;
}>;

/** UI 节点允许选择的强类型操作类别。 */
export type UiOperationKind = UiOperation['type'];

/** AQL 语义与执行后端选择分离的动作目标。 */
export type AutomationTarget = {
  /** 当前上下文或显式应用会话作用域。 */
  scope: TargetScope;
  /** 跨平台定位契约。 */
  locator: TargetLocator;
  /** 候选后端的 allow/deny 集合与稳定偏好顺序。 */
  backend_policy: BackendPolicy;
};

export type WorkflowInputType = 'text';

/** 一个必须由每次运行单独提供的输入声明。 */
export type WorkflowInputDefinition = {
  key: string;
  value_type: WorkflowInputType;
};

/** 一次运行的瞬时输入，不写回工作流定义。 */
export type RunInputs = {
  values: JsonObject;
};

/** 资源引用与普通 JSON 值引用保持独立。 */
export type ResourceRef = {
  producer_node_id: string;
  output_name: string;
};

/** UI 操作使用的逻辑资源作用域。 */
export type TargetScope =
  | { type: 'current' }
  | { type: 'application'; resource: ResourceRef }
  | { type: 'browser'; resource: ResourceRef };

/** 结构化引用的稳定来源。 */
export type ValueSource =
  | { type: 'workflow_input'; key: string }
  | { type: 'variable'; name: string }
  | { type: 'node'; node_id: string };

/** 节点参数的数据来源或纯计算表达式。 */
export type ValueExpr =
  | { type: 'literal'; value: JsonValue }
  | { type: 'ref'; source: ValueSource; pointer: string }
  | { type: 'expression'; source: string };

export type ValueExprKind = ValueExpr['type'];

/** 编辑器用于列出已知节点输出的只读展示描述。 */
export type ValueOutputDescriptor = Readonly<{
  name: string;
  valueType: 'text' | 'json';
  label: string;
}>;

/** Set Variables 节点中的一个显式、事务式赋值。 */
export type VariableAssignment = Readonly<{
  name: string;
  value: ValueExpr;
}>;

/** 查询规划时独立于 AQL 语义的开放后端集合策略。 */
export type BackendPolicy = {
  /** 空数组表示允许全部已注册后端。 */
  allow: BackendKind[];
  /** deny 始终覆盖 allow。 */
  deny: BackendKind[];
  /** 从高到低排列；未列出的候选沿用 Planner 稳定顺序。 */
  prefer: BackendKind[];
};

/** 与 workflow schema 独立演进的持久化 AQL 源码。 */
export type AqlQuery = {
  language_version: 1 | 2;
  source: string;
  /** 参数值独立于源码保存，Runtime prepare 时按文本类型冻结。 */
  bindings?: Readonly<Record<string, ValueExpr>>;
};

/** AQL、视觉查询、物理坐标或当前键盘焦点组成的目标判别联合。 */
export type TargetLocator =
  | { type: 'query'; query: AqlQuery }
  | { type: 'visual'; query: VisualQueryExpr }
  | { type: 'coordinate'; point: { x: number; y: number } }
  | { type: 'focused' };

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

/** 工作流对所有高风险系统路径的开放能力授权。 */
export type WorkflowCapabilityId = string;

export type WorkflowPermissions = {
  /** 未列出的能力一律拒绝。 */
  allow: readonly WorkflowCapabilityId[];
};

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

/** 分支节点注册的开放控制流端口；内置 Condition 使用 true/false。 */
export type ControlPortId = string;

/** 有向边；只有 Condition 源节点携带 branch。 */
export type WorkflowEdgeContract = {
  /** 文档内唯一 ID。 */
  id: string;
  /** 源节点 ID。 */
  source: string;
  /** 目标节点 ID。 */
  target: string;
  /** Condition 源节点的分支。 */
  branch: ControlPortId | null;
};

/** Runtime 内置校验问题码。 */
export type BuiltinValidationIssueCode =
  | 'unsupported_schema_version'
  | 'empty_workflow_name'
  | 'invalid_workflow_inputs'
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
  | 'invalid_browser_spec'
  | 'application_permission_denied'
  | 'invalid_backend_policy'
  | 'invalid_target_wait_policy'
  | 'invalid_command'
  | 'command_permission_denied'
  | 'invalid_value_reference'
  | 'invalid_expression'
  | 'invalid_output_binding'
  | 'invalid_variable_assignment'
  | 'invalid_resource_reference'
  | 'reference_not_dominating'
  | 'unknown_node_type'
  | 'invalid_node_definition';

/** 内置问题码或由注册节点拥有的命名空间化问题码。 */
export type ValidationIssueCode = BuiltinValidationIssueCode
  | (string & Readonly<Record<never, never>>);

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

/** 本地 Run Trace 的稳定生命周期状态。 */
export type RunStatus = 'starting' | 'running' | 'completed' | 'failed' | 'crashed';
/** 单次运行保存的诊断详细程度。 */
export type RunTraceLevel = 'off' | 'basic' | 'diagnostics' | 'forensics';

/** Run List 无需扫描 JSONL 即可展示的轻量索引。 */
export type RunManifest = {
  schema_version: 1;
  run_id: string;
  workflow_id: string;
  workflow_name: string;
  started_at_unix_ms: number;
  finished_at_unix_ms: number | null;
  status: RunStatus;
  trace_level: RunTraceLevel;
  event_count: number;
  trace_degraded: boolean;
  failed_node_id: string | null;
  failure_message: string | null;
};

/** 一次历史运行与其执行时工作流快照。 */
export type RunDetails = {
  manifest: RunManifest;
  workflow: WorkflowDefinition;
  nodes: RunNodeTrace[];
  artifacts: RunArtifactSummary[];
  query_traces: VisualQueryTrace[];
};

export type RunArtifactKind = 'captured_frame' | 'ocr_source_roi' | 'ocr_model_input';
export type RunArtifactSummary = {
  artifact_id: string;
  kind: RunArtifactKind;
  mime_type: string;
  width: number | null;
  height: number | null;
  request_id: string | null;
  frame_id: number;
};

export type VisualSelectionOutcome =
  | 'not_found' | 'unique' | 'ambiguous' | 'rejected_confidence';
export type VisualQueryTrace = {
  run_id: string;
  node_id: string;
  scene_id: number;
  frame_id: number;
  query: string;
  outcome: VisualSelectionOutcome;
  candidates: Array<{
    raw_text: string;
    bbox: { x: number; y: number; width: number; height: number };
    confidence: number;
    source: string;
  }>;
  minimum_click_confidence: number;
  selected_candidate_index: number | null;
  send_input_blocked: boolean;
};

/** 节点输入在 Runtime 中的事实来源。 */
export type ResolvedInputSource =
  | { type: 'literal' }
  | { type: 'workflow_input'; key: string }
  | { type: 'variable'; name: string }
  | { type: 'node'; node_id: string }
  | { type: 'expression'; expression: string }
  | { type: 'resource'; producer_node_id: string; output_name: string };

export type ResolvedInputField = {
  name: string;
  expected_type: string;
  source: ResolvedInputSource;
  value: JsonValue | null;
  redacted: boolean;
};

export type ResolvedNodeInputs = {
  schema_version: 1;
  node_id: string;
  node_sequence: number;
  fields: ResolvedInputField[];
};

export type RunNodeTrace = {
  node_id: string;
  node_sequence: number;
  resolved_inputs: ResolvedNodeInputs;
  outputs: {
    schema_version: 1;
    output_names: string[];
    resource_names: string[];
  } | null;
};

export type ExecutionEventKind =
  | 'workflow_started' | 'node_started' | 'log' | 'node_output_produced'
  | 'resource_acquired' | 'backend_selected' | 'command_exited'
  | 'diagnostic_evidence_captured' | 'node_succeeded'
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
  /** 组件内部事件对应的扁平执行节点 ID。 */
  expanded_node_id?: string | null;
  /** 组件内部事件从外到内的版本锁定来源路径。 */
  component_path?: ExecutionComponentFrame[];
  /** 相关连线 ID。 */
  edge_id: string | null;
  /** 生命周期类别。 */
  kind: ExecutionEventKind;
  /** 可选运行说明。 */
  message: string | null;
  /** 不包含业务输出或 OS handle 的结构化载荷。 */
  payload: ExecutionEventPayload | null;
};

/** JSONL 中包裹产品执行事件的诊断 envelope。 */
export type RunTraceEvent = {
  schema_version: 1;
  trace_sequence: number;
  timestamp_unix_ms: number;
  event: ExecutionEvent;
};

export type ExecutionEventPayload =
  | { type: 'node_outputs_produced'; output_names: string[] }
  | { type: 'resource_acquired'; output_name: string; resource_type: string }
  | { type: 'backend_selected'; backend: BackendKind }
  | { type: 'command_exited'; exit_code: number }
  | {
      type: 'diagnostic_evidence_captured';
      evidence_id: string;
      backend: BackendKind;
      branch_path: number[];
      recovered_by_fallback: boolean;
    };

/** Rust `CommandErrorCode` 的完整序列化取值。 */
export const COMMAND_ERROR_CODES = [
  'validation_failed',
  'event_delivery_failed',
  'execution_invariant_failed',
  'automation_failed',
  'application_failed',
  'browser_failed',
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
export type {
  BrowserOperation, BrowserSpec, ComponentInstance, ComponentValueOutput,
  DelimitedTextFormat, ExecutionComponentFrame, FlowComponentDefinition,
} from '../components/reusableFlowContracts';
