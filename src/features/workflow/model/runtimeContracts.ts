import type { ExecutionStructureFrame } from '../components/reusableFlowContracts';
import type { BackendKind } from './aqlContracts';
import type { JsonValue } from './jsonContracts';
import type { WorkflowDefinition } from './contracts';

/** 本地 Run Trace 的稳定生命周期状态。 */
export type RunStatus = 'starting' | 'running' | 'completed' | 'failed' | 'crashed';
/** 单次运行保存的诊断详细程度。 */
export type RunTraceLevel = 'off' | 'basic' | 'diagnostics' | 'forensics';

/** 工作流 schema 之外、在运行启动时冻结的最小展示快照。 */
export type RunPresentationSnapshot = {
  schema_version: 1;
  node_labels: Readonly<Record<string, string>>;
};

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
  presentation: RunPresentationSnapshot;
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
  node_sequence: number;
  window_handle: string;
  frame_id: number;
};

export type VisualSelectionOutcome =
  | 'not_found' | 'unique' | 'multiple' | 'ambiguous' | 'rejected_confidence';
export type VisualQueryTrace = {
  schema_version: 2;
  run_id: string;
  node_id: string;
  node_sequence: number;
  query: string;
  outcome: VisualSelectionOutcome;
  candidate_nodes: SceneNodeRef[];
  selected_node: SceneNodeRef | null;
  metrics: {
    elapsed_us: number;
    exact_index_hits: number;
    scanned_nodes: number;
    spatial_candidates: number;
  };
  projection: {
    schema_version: 2;
    windows: SceneWindowProjection[];
    nodes: SceneNodeProjection[];
  };
};

export type PixelRect = { x: number; y: number; width: number; height: number };
export type SceneNodeRef = {
  window_handle: string;
  scene_id: number;
  node_id: string;
};
export type SceneWindowProjection = {
  window_handle: string;
  scene_id: number;
  frame_id: number;
  z_order: number;
  foreground: boolean;
  screen_bounds: PixelRect;
  frame_bounds: PixelRect;
};

export type SceneNodeProjection = {
  node_id: string;
  scene_id: number;
  frame_id: number;
  window_handle: string;
  text: string;
  frame_bbox: PixelRect;
  screen_bbox: PixelRect;
  polygon: Array<{ x: number; y: number }>;
  confidence: number;
  source: string;
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
  | 'diagnostic_evidence_captured' | 'observation_evaluated'
  | 'loop_started' | 'loop_iteration' | 'loop_exhausted' | 'loop_completed'
  | 'workflow_failure_declared' | 'node_succeeded'
  | 'edge_traversed' | 'node_failed' | 'workflow_completed' | 'workflow_failed';

export type ExecutionEvent = {
  run_id: string;
  workflow_id: string;
  sequence: number;
  node_id: string | null;
  expanded_node_id?: string | null;
  structure_path?: ExecutionStructureFrame[];
  edge_id: string | null;
  kind: ExecutionEventKind;
  message: string | null;
  payload: ExecutionEventPayload | null;
};

/** JSONL 中包裹产品执行事件的诊断 envelope。 */
export type RunTraceEvent = {
  schema_version: 1;
  trace_sequence: number;
  timestamp_unix_ms: number;
  event: ExecutionEvent;
};

export type ObservationValueType = 'entities' | 'records' | 'number' | 'boolean';

export type ExecutionEventPayload =
  | { type: 'node_outputs_produced'; output_names: string[] }
  | { type: 'resource_acquired'; output_name: string; resource_type: string }
  | { type: 'backend_selected'; backend: BackendKind }
  | { type: 'command_exited'; exit_code: number }
  | {
      type: 'observation_evaluated';
      value_type: ObservationValueType;
      backend: BackendKind | null;
      known: boolean;
    }
  | { type: 'loop_started'; scope_id: string; max_iterations: number }
  | { type: 'loop_iteration'; iteration: number; max_iterations: number }
  | { type: 'loop_exhausted'; iterations: number }
  | { type: 'loop_completed'; iterations: number }
  | { type: 'workflow_failure_declared'; code: string }
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
  'workflow_failed',
  'runtime_data_failed',
] as const;

export type BackendCommandErrorCode = typeof COMMAND_ERROR_CODES[number];
export type CommandErrorCode = BackendCommandErrorCode | 'unknown_error';

export type CommandError<TIssue = unknown> = {
  code: CommandErrorCode;
  message: string;
  issues: readonly TIssue[];
};
