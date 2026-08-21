import {
  Clock3,
  FileText,
  GitBranch,
  PlayCircle,
  Square,
  type LucideIcon,
} from 'lucide-react';

import type { FlowNodeRendererProps, NodeDefinition } from '../../flow';
import {
  WORKFLOW_NODE_SIZES,
  type NodeRunState,
  type WorkflowNodeData,
} from '../../features/workflow/workflowModel';

type WorkflowNodeKind = WorkflowNodeData['kind'];

type NodeSize = Readonly<{
  width: number;
  height: number;
}>;

type WorkflowNodeRegistry = Record<
  WorkflowNodeKind,
  NodeDefinition<WorkflowNodeData>
>;

/** 画布节点类型对应的统一线性图标。 */
const NODE_ICONS: Readonly<Record<WorkflowNodeKind, LucideIcon>> = {
  start: PlayCircle,
  log: FileText,
  delay: Clock3,
  condition: GitBranch,
  end: Square,
};

/** 节点类型对应的边框、图标底色和阴影。 */
const NODE_TONES: Readonly<Record<
  WorkflowNodeKind,
  Readonly<{ card: string; icon: string }>
>> = {
  start: {
    card: 'border-emerald-500 shadow-[0_4px_14px_rgba(16,185,129,.10)]',
    icon: 'bg-emerald-50 text-emerald-600',
  },
  log: {
    card: 'border-blue-500 shadow-[0_4px_14px_rgba(59,130,246,.10)]',
    icon: 'bg-blue-50 text-blue-600',
  },
  delay: {
    card: 'border-amber-500 shadow-[0_4px_14px_rgba(245,158,11,.10)]',
    icon: 'bg-amber-50 text-amber-600',
  },
  condition: {
    card: 'border-violet-500 shadow-[0_4px_14px_rgba(139,92,246,.10)]',
    icon: 'bg-violet-50 text-violet-600',
  },
  end: {
    card: 'border-rose-500 shadow-[0_4px_14px_rgba(244,63,94,.10)]',
    icon: 'bg-rose-50 text-rose-600',
  },
};

/** 执行状态点对应的颜色和动画。 */
const STATUS_TONES: Readonly<Record<NodeRunState, string>> = {
  idle: 'bg-slate-400',
  running: 'animate-pulse bg-blue-500',
  success: 'bg-emerald-500',
  error: 'bg-rose-500',
};

/** ArgusFlow 节点注册表，由通用 Flow 内核按 kind 分派。 */
export const workflowNodeRegistry = {
  start: createDefinition('start', '开始', WORKFLOW_NODE_SIZES.start, true),
  log: createDefinition('log', '日志', WORKFLOW_NODE_SIZES.log),
  delay: createDefinition('delay', '等待', WORKFLOW_NODE_SIZES.delay),
  condition: createDefinition('condition', '条件', WORKFLOW_NODE_SIZES.condition),
  end: createDefinition('end', '结束', WORKFLOW_NODE_SIZES.end, true),
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
    defaultSize: { ...defaultSize },
    singleton,
    component: WorkflowNodeCard,
  };
}

/** 根据节点类型渲染与参考图一致的白底彩框业务卡片。 */
export function WorkflowNodeCard({ node }: FlowNodeRendererProps<WorkflowNodeData>) {
  const data = node.data;
  const detail = resolveNodeDetail(data);
  const status = data.runState ?? 'idle';
  const tone = NODE_TONES[data.kind];
  const invalidTone = data.invalid ? 'ring-2 ring-rose-200' : '';
  const Icon = NODE_ICONS[data.kind];

  return (
    <div
      className={`flex h-full w-full items-center gap-2 rounded-[7px] border bg-white px-2.5 ${tone.card} ${invalidTone}`}
    >
      <span className={`flex size-7 shrink-0 items-center justify-center rounded-full ${tone.icon}`}>
        <Icon className="size-[18px] stroke-[1.8]" aria-hidden="true" />
      </span>
      <div className="flex min-w-0 flex-1 flex-col justify-center text-slate-800">
        <strong className="truncate text-[12px] leading-[18px] font-semibold">{data.label}</strong>
        <span className="truncate text-[10px] leading-[15px] text-slate-500">{detail}</span>
      </div>
      {status !== 'idle' ? (
        <span className={`size-1.5 shrink-0 rounded-full ${STATUS_TONES[status]}`} />
      ) : null}
    </div>
  );
}

/** 将节点专属配置压缩为卡片副标题。 */
function resolveNodeDetail(data: WorkflowNodeData): string {
  switch (data.kind) {
    case 'log':
      return data.message ?? '';
    case 'delay':
      return `等待 ${(data.milliseconds ?? 0) / 1000} 秒`;
    case 'condition':
      return '检测数据量';
    case 'start':
      return '手动触发';
    case 'end':
      return '流程结束';
  }
}
