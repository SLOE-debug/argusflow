import type { FlowEdgeLabel } from '../../../flow';

/** 当前工作流内置分支的稳定键。 */
export type WorkflowBranchKey =
  | 'true'
  | 'false'
  | 'unknown'
  | 'known'
  | 'iterate'
  | 'exhausted';

/** 分支标签同时提供清晰文案和非纯红色的语义色。 */
export const WORKFLOW_BRANCH_PRESENTATIONS = {
  true: { text: '条件成立', color: '#15803d' },
  false: { text: '条件不成立', color: '#dc2626' },
  unknown: { text: '暂无法判断', color: '#d97706' },
  known: { text: '已获取结果', color: '#2563eb' },
  iterate: { text: '进入下一轮', color: '#7c3aed' },
  exhausted: { text: '已达上限', color: '#ea580c' },
} as const satisfies Readonly<Record<WorkflowBranchKey, FlowEdgeLabel>>;

/** 从业务边数据读取内置分支；普通顺序连线不显示标签。 */
export function resolveWorkflowEdgeLabel(data: unknown): FlowEdgeLabel | null {
  if (typeof data !== 'object' || data === null || !('branch' in data)) return null;
  return isWorkflowBranchKey(data.branch)
    ? WORKFLOW_BRANCH_PRESENTATIONS[data.branch]
    : null;
}

/** 避免用无约束断言访问类型化分支表。 */
function isWorkflowBranchKey(value: unknown): value is WorkflowBranchKey {
  return typeof value === 'string'
    && Object.prototype.hasOwnProperty.call(WORKFLOW_BRANCH_PRESENTATIONS, value);
}
