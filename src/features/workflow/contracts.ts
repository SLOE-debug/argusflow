/** 可在前后端无损传递的 JSON 值。 */
export type JsonValue = null | boolean | number | string | JsonValue[] | { [key: string]: JsonValue };
export type JsonObject = { [key: string]: JsonValue };

/** 与 Rust 后端交换的 schema v2 工作流。 */
export type WorkflowDefinition = {
  /** 当前契约固定版本。 */
  schema_version: 2;
  /** 工作流稳定 ID。 */
  id: string;
  /** 面向用户的名称。 */
  name: string;
  /** Condition 读取的只读 JSON 对象。 */
  variables: JsonObject;
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
  | { type: 'delay'; milliseconds: number }
  | { type: 'condition'; predicate: ConditionPredicate }
  | { type: 'action'; action: AutomationAction }
  | { type: 'end' };

export type AutomationAction =
  | { type: 'click'; target: Selector }
  | { type: 'set_value'; target: Selector; value: string };

export type Selector =
  | { type: 'native'; name: string | null; automation_id: string | null; control_type: string | null }
  | { type: 'browser'; css: string }
  | { type: 'visual_text'; text: string; exact: boolean }
  | { type: 'coordinate'; x: number; y: number };

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
  | 'invalid_delay';

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
  | 'workflow_started' | 'node_started' | 'log' | 'node_succeeded'
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
};

/** Rust `CommandErrorCode` 的完整序列化取值。 */
export const COMMAND_ERROR_CODES = [
  'validation_failed',
  'run_in_progress',
  'event_delivery_failed',
  'execution_invariant_failed',
  'automation_failed',
] as const;

export type BackendCommandErrorCode = typeof COMMAND_ERROR_CODES[number];
export type CommandErrorCode = BackendCommandErrorCode | 'unknown_error';

export type CommandError = {
  code: CommandErrorCode;
  message: string;
  issues: readonly ValidationIssue[];
};
