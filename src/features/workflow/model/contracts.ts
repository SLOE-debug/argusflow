import type { BackendKind } from './aqlContracts';
import type { KeyChord } from './inputContracts';
import type { JsonObject, JsonValue } from './jsonContracts';
export type * from './aqlContracts';
export { isJsonObject, type JsonObject, type JsonValue } from './jsonContracts';
export type {
  TargetWaitMode,
  TargetWaitPolicy,
  UiExecutionPolicy,
} from './uiExecutionContracts';

/** 与 Rust 后端交换的 schema v10 多作用域工作流。 */
export type WorkflowDefinition = {
  /** 当前契约固定版本。 */
  schema_version: 10;
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
  /** 根流程与全部 While 子作用域。 */
  graph: ScopedFlowGraphContract;
};
export type Position = { x: number; y: number };
export type Size = { width: number; height: number };
/** 后端可执行节点的通用字段与开放 definition envelope。 */
export type WorkflowNodeContract = {
  id: string;
  position: Position;
  /** 节点或结构容器的持久化逻辑尺寸。 */
  size: Size;
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
  | { type: 'type_text'; target: AutomationTarget; value: ValueExpr };

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
  language_version: 3;
  source: string;
  /** 参数值独立于源码保存，Runtime prepare 时按推导类型冻结。 */
  bindings: Readonly<Record<string, ValueExpr>>;
};

/** Observe 结果的静态顶层类型。 */
export type ObservationValueType = 'entities' | 'records' | 'number' | 'boolean';

/** Unknown 只允许在明确总预算内重试。 */
export type ObservationPolicy =
  | { mode: 'once' }
  | { mode: 'bounded'; timeout_ms: number; poll_interval_ms: number };

/** 通用观察节点保存的事实来源、AQL v3 与有限等待策略。 */
export type ObserveSpec = Readonly<{
  scope: TargetScope;
  query: AqlQuery;
  backend_policy: BackendPolicy;
  policy: ObservationPolicy;
}>;

/** AQL、物理坐标或当前键盘焦点组成的目标判别联合。 */
export type TargetLocator =
  | { type: 'query'; query: AqlQuery }
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

/** 多作用域图中的根、组件或 While 固定边界。 */
export type FlowScopeBoundaryContract =
  | Readonly<{ type: 'workflow'; entry_node_id: string }>
  | Readonly<{ type: 'component'; entry_node_id: string; exit_node_id: string }>
  | Readonly<{
      type: 'loop';
      entry_node_id: string;
      continue_node_id: string;
      complete_node_id: string;
    }>;

/** While 子作用域的直接父容器。 */
export type FlowScopeParentContract = Readonly<{
  scope_id: string;
  node_id: string;
}>;

/** 一份作用域内独立的 DAG 文档。 */
export type FlowScopeContract = Readonly<{
  id: string;
  parent: FlowScopeParentContract | null;
  boundary: FlowScopeBoundaryContract;
  nodes: WorkflowNodeContract[];
  edges: WorkflowEdgeContract[];
}>;

/** 工作流和组件共享的扁平作用域表。 */
export type ScopedFlowGraphContract = Readonly<{
  root_scope_id: string;
  scopes: FlowScopeContract[];
}>;

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
  | 'invalid_observation_policy'
  | 'invalid_loop'
  | 'invalid_failure'
  | 'invalid_aql_query'
  | 'invalid_application_spec'
  | 'invalid_browser_spec'
  | 'application_permission_denied'
  | 'invalid_backend_policy'
  | 'invalid_target_wait_policy'
  | 'invalid_extract'
  | 'invalid_data_format'
  | 'invalid_command'
  | 'command_permission_denied'
  | 'invalid_value_reference'
  | 'undeclared_variable'
  | 'invalid_expression'
  | 'invalid_output_binding'
  | 'invalid_variable_assignment'
  | 'invalid_resource_reference'
  | 'reference_not_dominating'
  | 'unknown_node_type'
  | 'invalid_node_definition'
  | 'invalid_scope'
  | 'invalid_scope_boundary'
  | 'invalid_node_size';

/** 内置问题码或由注册节点拥有的命名空间化问题码。 */
export type ValidationIssueCode = BuiltinValidationIssueCode
  | (string & Readonly<Record<never, never>>);

export type ValidationIssue = {
  code: ValidationIssueCode;
  message: string;
  node_id: string | null;
  edge_id: string | null;
  scope_id?: string | null;
  structure_path?: string[];
};

export type ValidationReport = {
  valid: boolean;
  issues: readonly ValidationIssue[];
};
export type RunStarted = { run_id: string };

export type {
  BackendCommandErrorCode, CommandErrorCode, ExecutionEvent, ExecutionEventKind,
  ExecutionEventPayload, ResolvedInputField, ResolvedInputSource, ResolvedNodeInputs,
  RunArtifactKind, RunArtifactSummary, RunDetails, RunManifest, RunNodeTrace, RunStatus,
  RunPresentationSnapshot, RunTraceEvent, RunTraceLevel, SceneNodeProjection, SceneNodeRef,
  SceneWindowProjection, PixelRect, VisualQueryTrace,
  VisualSelectionOutcome,
} from './runtimeContracts';
export type CommandError = import('./runtimeContracts').CommandError<ValidationIssue>;
export { COMMAND_ERROR_CODES } from './runtimeContracts';
export type {
  BrowserOperation, BrowserSpec, ComponentInstance, ComponentValueOutput,
  DelimitedTextFormat, ExecutionComponentFrame, ExecutionLoopFrame,
  ExecutionStructureFrame, FlowComponentDefinition,
} from '../components/reusableFlowContracts';
