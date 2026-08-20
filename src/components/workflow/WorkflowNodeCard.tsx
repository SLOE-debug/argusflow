import { Clock3, GitBranch, List, Play, Square, type LucideIcon } from 'lucide-react';

import type { FlowNodeRendererProps, NodeDefinition } from '../../flow';
import type {
  NodeRunState,
  WorkflowNodeData,
} from '../../features/workflow/workflowModel';

type WorkflowNodeKind = WorkflowNodeData['kind'];

type NodeSize = {
  readonly width: number;
  readonly height: number;
};

type WorkflowNodeRegistry = Record<
  WorkflowNodeKind,
  NodeDefinition<WorkflowNodeData>
>;

/** 画布节点类型对应的统一线性图标。 */
const icons: Record<WorkflowNodeKind, LucideIcon> = {
  start: Play,
  log: List,
  delay: Clock3,
  condition: GitBranch,
  end: Square,
};

/** 画布节点类型对应的强调色与浅色渐变。 */
const nodeTones: Record<WorkflowNodeKind, string> = {
  start: 'text-emerald-600 from-emerald-50',
  end: 'text-rose-600 from-rose-50',
  condition: 'text-violet-600 from-violet-50',
  delay: 'text-orange-600 from-orange-50',
  log: 'text-blue-600 from-blue-50',
};

/** 执行状态点对应的颜色和动画。 */
const statusTones: Record<NodeRunState, string> = {
  idle: 'bg-slate-400',
  running: 'animate-pulse bg-blue-500 ring-4 ring-blue-100',
  success: 'bg-emerald-500',
  error: 'bg-rose-500',
};

/** ArgusFlow 节点注册表，由通用 Flow 内核按 kind 分派。 */
export const workflowNodeRegistry = {
  start: createDefinition('start', '开始', { width: 168, height: 68 }, true),
  log: createDefinition('log', '日志', { width: 200, height: 72 }),
  delay: createDefinition('delay', '等待', { width: 200, height: 72 }),
  condition: createDefinition('condition', '条件', { width: 200, height: 72 }),
  end: createDefinition('end', '结束', { width: 168, height: 68 }, true),
} satisfies WorkflowNodeRegistry;

/** 构造带统一业务渲染器的节点定义。 */
function createDefinition(
  kind: WorkflowNodeKind,
  title: string,
  defaultSize: NodeSize,
  singleton = false,
): NodeDefinition<WorkflowNodeData> {
  return {
    kind,
    title,
    defaultSize,
    singleton,
    component: WorkflowNodeCard,
  };
}

/** 根据节点类型渲染矩形业务卡片。 */
export function WorkflowNodeCard({ node }: FlowNodeRendererProps<WorkflowNodeData>) {
  const data = node.data;
  const detail = resolveNodeDetail(data);
  const status = data.runState ?? 'idle';
  const invalidTone = data.invalid
    ? 'border-rose-500 ring-[3px] ring-rose-100'
    : 'border-slate-300';
  const Icon = icons[data.kind];
  const cardClassName = [
    'relative flex h-full w-full items-center gap-3 overflow-hidden rounded-xl border',
    'bg-gradient-to-br to-white px-3 py-2',
    'shadow-[0_7px_18px_rgba(43,60,82,.10),0_1px_2px_rgba(43,60,82,.08)]',
    'transition-shadow hover:shadow-[0_10px_24px_rgba(43,60,82,.13)]',
    nodeTones[data.kind],
    invalidTone,
  ].join(' ');

  return (
    <div className={cardClassName}>
      <span className="absolute inset-y-2 left-0 w-[3px] rounded-r bg-current" />
      <Icon
        className="size-5 shrink-0 stroke-[1.9]"
        aria-hidden="true"
      />
      <div className="flex min-w-0 flex-1 flex-col justify-center text-slate-800">
        <strong className="truncate text-sm leading-tight">{data.label}</strong>
        <span className="mt-0.5 truncate text-[11px] leading-tight text-slate-500">
          {detail}
        </span>
      </div>
      <span
        className={
          'size-2 shrink-0 rounded-full border-2 border-white ' +
          `shadow-[0_0_0_1px_rgba(88,105,128,.18)] ${statusTones[status]}`
        }
      />
    </div>
  );
}

/** 将节点专属配置压缩为卡片副标题。 */
function resolveNodeDetail(data: WorkflowNodeData): string {
  switch (data.kind) {
    case 'log':
      return data.message ?? '';
    case 'delay':
      return `${data.milliseconds ?? 0} ms`;
    case 'condition':
      return `${data.pointer || '/'} · ${data.operator}`;
    case 'start':
      return '流程入口';
    case 'end':
      return '流程出口';
  }
}
