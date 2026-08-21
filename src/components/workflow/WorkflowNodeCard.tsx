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

/** 节点类型对应的强调色条和图标底色。 */
const NODE_TONES: Readonly<Record<
  WorkflowNodeKind,
  Readonly<{ accent: string; icon: string }>
>> = {
  start: {
    accent: 'bg-emerald-500',
    icon: 'bg-emerald-50 text-emerald-600',
  },
  log: {
    accent: 'bg-blue-500',
    icon: 'bg-blue-50 text-blue-600',
  },
  delay: {
    accent: 'bg-amber-500',
    icon: 'bg-amber-50 text-amber-600',
  },
  condition: {
    accent: 'bg-violet-500',
    icon: 'bg-violet-50 text-violet-600',
  },
  end: {
    accent: 'bg-rose-500',
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

/** 根据节点类型渲染带左侧强调色条和完整选中状态的紧凑业务卡片。 */
export function WorkflowNodeCard({
  node,
  selected,
}: FlowNodeRendererProps<WorkflowNodeData>) {
  const data = node.data;
  const detail = resolveNodeDetail(data);
  const status = data.runState ?? 'idle';
  const tone = NODE_TONES[data.kind];
  const invalidTone = data.invalid ? 'ring-2 ring-rose-200' : '';
  /** 选中态覆盖卡片表面、边框和阴影，但保留节点类型强调色。 */
  const selectedSurfaceTone = selected
    ? 'border-blue-400 bg-blue-50 shadow-[0_4px_14px_rgba(37,99,235,0.14)]'
    : 'border-slate-200 bg-white shadow-[0_3px_10px_rgba(15,23,42,0.08)]';
  /** 选中卡片使用更明确的蓝色文字层级。 */
  const selectedTextTone = selected ? 'text-blue-950' : 'text-slate-800';
  const selectedDetailTone = selected ? 'text-blue-600' : 'text-slate-400';
  const Icon = NODE_ICONS[data.kind];

  return (
    <div
      className={`relative flex h-full w-full items-center gap-2 overflow-hidden rounded-lg border pr-2.5 pl-3 ${selectedSurfaceTone} ${invalidTone}`}
      data-selected={selected ? 'true' : 'false'}
    >
      <span className={`absolute inset-y-0 left-0 w-1 ${tone.accent}`} aria-hidden="true" />
      <span className={`flex size-6 shrink-0 items-center justify-center rounded-md ${tone.icon}`}>
        <Icon className="size-3.5 stroke-[1.9]" aria-hidden="true" />
      </span>
      <div className={`flex min-w-0 flex-1 flex-col justify-center ${selectedTextTone}`}>
        <strong className="truncate text-[12px] leading-4 font-semibold">{data.label}</strong>
        <span className={`truncate text-[10px] leading-[14px] ${selectedDetailTone}`}>{detail}</span>
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
