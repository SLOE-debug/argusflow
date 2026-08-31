import type {
  BackendKind,
  ExecutionEvent,
  ExecutionEventKind,
} from '../model/contracts';
import type {
  WorkflowCanvasNode,
  WorkflowNodeData,
} from '../model/workflowModel';

/** 日志事件自身的结果语义，与节点身份色彼此独立。 */
export type ExecutionLogSeverity =
  | 'normal'
  | 'success'
  | 'warning'
  | 'error';

/** 单条后端执行事件对应的中文、高密度日志展示模型。 */
export type ExecutionLogEntry = Readonly<{
  /** 后端在运行实例内分配的严格递增序号。 */
  sequence: number;
  /** 相关节点 ID；流程级事件为 null。 */
  nodeId: string | null;
  /** 用户在当前工作流中配置的节点名称。 */
  nodeLabel: string | null;
  /** 节点类型用于复用画布身份色。 */
  nodeKind: WorkflowNodeData['kind'] | null;
  /** 面向用户的中文事件名称。 */
  eventLabel: string;
  /** 保留用户输出并产品化系统载荷后的事件详情。 */
  detail: string;
  /** 事件结果语义，用于图标或事件文案颜色。 */
  severity: ExecutionLogSeverity;
}>;

/** 后端稳定事件枚举对应的中文产品文案。 */
export const EXECUTION_EVENT_LABELS = {
  workflow_started: '开始运行',
  node_started: '开始执行',
  log: '记录信息',
  node_output_produced: '输出结果',
  resource_acquired: '资源已准备',
  backend_selected: '已选择执行方式',
  command_exited: '命令已完成',
  diagnostic_evidence_captured: '已保存诊断信息',
  observation_evaluated: '检查完成',
  loop_iteration: '开始下一轮',
  loop_exhausted: '已达上限',
  workflow_failure_declared: '流程已停止',
  node_succeeded: '执行完成',
  edge_traversed: '进入下一步',
  node_failed: '执行失败',
  workflow_completed: '运行完成',
  workflow_failed: '运行失败',
} as const satisfies Readonly<Record<ExecutionEventKind, string>>;

/** Runtime Planner 后端枚举对应的产品名称。 */
export const BACKEND_LABELS = {
  windows_uia: 'Windows 控件',
  browser_cdp: '网页元素',
  ocr_small: '屏幕文字识别',
  send_input: '模拟键盘输入',
} as const satisfies Readonly<Record<BackendKind, string>>;

/** 将单条稳定协议事件转换为不泄漏内部枚举的产品展示模型。 */
export function resolveExecutionLogEntry(
  event: ExecutionEvent,
  nodes: ReadonlyArray<WorkflowCanvasNode>,
): ExecutionLogEntry {
  const node = event.node_id
    ? nodes.find((candidate) => candidate.id === event.node_id) ?? null
    : null;

  return {
    sequence: event.sequence,
    nodeId: event.node_id,
    nodeLabel: node?.data.label ?? null,
    nodeKind: node?.data.kind ?? null,
    eventLabel: EXECUTION_EVENT_LABELS[event.kind],
    detail: resolveExecutionDetail(event),
    severity: resolveExecutionSeverity(event.kind),
  };
}

/** 把展示模型格式化为默认复制到剪贴板的人类可读文本。 */
export function formatExecutionLogEntry(entry: ExecutionLogEntry): string {
  const sequence = String(entry.sequence).padStart(2, '0');
  const node = entry.nodeLabel
    ? `[${entry.nodeLabel}] `
    : entry.nodeId
      ? `[节点 ${entry.nodeId}] `
      : '';
  const detail = entry.detail ? ` ${entry.detail}` : '';
  return `${sequence} ${node}${entry.eventLabel}${detail}`;
}

/** 系统事件使用结构化载荷，用户 Log/Debug 文本则保持后端 message 原样。 */
function resolveExecutionDetail(event: ExecutionEvent): string {
  switch (event.payload?.type) {
    case 'backend_selected': {
      const backend = BACKEND_LABELS[event.payload.backend];
      const outcome = event.message?.trim();
      return outcome && outcome !== event.payload.backend
        ? `${backend} · ${outcome}`
        : backend;
    }
    case 'command_exited':
      return `退出代码 ${event.payload.exit_code}`;
    case 'node_outputs_produced':
      return event.message ?? (
        event.payload.output_names.length > 0
          ? `已产生输出：${event.payload.output_names.join('、')}`
          : '节点未产生值输出'
      );
    case 'resource_acquired':
      return event.message
        ?? '已准备好运行所需资源';
    case 'observation_evaluated':
      return event.message ?? (event.payload.known ? '已获取结果' : '暂无法判断');
    case 'loop_iteration':
      return event.message
        ?? `第 ${event.payload.iteration} / ${event.payload.max_iterations} 轮`;
    case 'loop_exhausted':
      return event.message ?? `达到设置的上限，共重复 ${event.payload.iterations} 次`;
    case 'workflow_failure_declared':
      return event.message ?? `错误标识：${event.payload.code}`;
    case 'diagnostic_evidence_captured':
      return event.message
        ?? '已保存失败诊断信息';
    default:
      break;
  }

  if (event.message) return event.message;
  switch (event.kind) {
    case 'node_started':
      return '正在执行';
    case 'node_succeeded':
      return '已完成';
    case 'edge_traversed':
      return '已进入下一步';
    case 'workflow_completed':
      return '流程已完成';
    case 'workflow_failed':
      return '流程运行失败';
    default:
      return '';
  }
}

/** 将生命周期类别压缩为 UI 所需的最小结果语义。 */
function resolveExecutionSeverity(
  kind: ExecutionEventKind,
): ExecutionLogSeverity {
  switch (kind) {
    case 'node_succeeded':
    case 'workflow_completed':
      return 'success';
    case 'diagnostic_evidence_captured':
    case 'loop_exhausted':
      return 'warning';
    case 'node_failed':
    case 'workflow_failure_declared':
    case 'workflow_failed':
      return 'error';
    default:
      return 'normal';
  }
}
