import {
  AppWindow,
  Bug,
  Braces,
  Globe2,
  CircleCheck,
  CircleX,
  Clock3,
  FileText,
  GitBranch,
  LoaderCircle,
  MinusCircle,
  MousePointerClick,
  PlayCircle,
  Square,
  Terminal,
  type LucideIcon,
} from 'lucide-react';

import type { FlowNodeRendererProps, NodeDefinition } from '../../flow';
import type {
  TargetLocatorKind,
  UiOperationKind,
  ValueExpr,
} from '../../features/workflow/contracts';
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
  debug: Bug,
  delay: Clock3,
  condition: GitBranch,
  variable: Braces,
  application: AppWindow,
  browser: Globe2,
  ui: MousePointerClick,
  command: Terminal,
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
  debug: {
    accent: 'bg-fuchsia-500',
    icon: 'bg-fuchsia-50 text-fuchsia-700',
  },
  delay: {
    accent: 'bg-amber-500',
    icon: 'bg-amber-50 text-amber-600',
  },
  condition: {
    accent: 'bg-violet-500',
    icon: 'bg-violet-50 text-violet-600',
  },
  variable: {
    accent: 'bg-teal-500',
    icon: 'bg-teal-50 text-teal-700',
  },
  application: {
    accent: 'bg-indigo-500',
    icon: 'bg-indigo-50 text-indigo-700',
  },
  browser: {
    accent: 'bg-sky-500',
    icon: 'bg-sky-50 text-sky-700',
  },
  ui: {
    accent: 'bg-cyan-500',
    icon: 'bg-cyan-50 text-cyan-700',
  },
  command: {
    accent: 'bg-slate-600',
    icon: 'bg-slate-100 text-slate-700',
  },
  end: {
    accent: 'bg-rose-500',
    icon: 'bg-rose-50 text-rose-600',
  },
};

type RuntimeTone = Readonly<{
  icon: LucideIcon | null;
  label: string;
  status: string;
}>;

/** 节点运行生命周期对应的稳定图标、文案和语义色。 */
const RUN_STATE_TONES: Readonly<Record<NodeRunState, RuntimeTone>> = {
  idle: { icon: null, label: '', status: '' },
  pending: { icon: Clock3, label: '待运行', status: 'text-slate-500' },
  running: { icon: LoaderCircle, label: '正在运行', status: 'text-blue-600' },
  success: { icon: CircleCheck, label: '已完成', status: 'text-emerald-600' },
  error: { icon: CircleX, label: '失败', status: 'text-rose-600' },
  skipped: { icon: MinusCircle, label: '未执行', status: 'text-slate-400' },
};

/** ArgusFlow 节点注册表，由通用 Flow 内核按 kind 分派。 */
export const workflowNodeRegistry = {
  start: {
    ...createDefinition('start', '开始', WORKFLOW_NODE_SIZES.start, true),
    canEndConnection: false,
  },
  log: createDefinition('log', '日志', WORKFLOW_NODE_SIZES.log),
  debug: createDefinition('debug', '调试输出', WORKFLOW_NODE_SIZES.debug),
  delay: createDefinition('delay', '等待', WORKFLOW_NODE_SIZES.delay),
  condition: createDefinition('condition', '条件', WORKFLOW_NODE_SIZES.condition),
  variable: createDefinition('variable', '设置变量', WORKFLOW_NODE_SIZES.variable),
  application: createDefinition('application', '应用', WORKFLOW_NODE_SIZES.application),
  browser: createDefinition('browser', '浏览器', WORKFLOW_NODE_SIZES.browser),
  ui: createDefinition('ui', '界面操作', WORKFLOW_NODE_SIZES.ui),
  command: createDefinition('command', '执行命令', WORKFLOW_NODE_SIZES.command),
  end: {
    ...createDefinition('end', '结束', WORKFLOW_NODE_SIZES.end, true),
    canStartConnection: false,
  },
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
  const runtimeTone = RUN_STATE_TONES[status];
  const StatusIcon = runtimeTone.icon;
  const invalidTone = data.invalid ? 'ring-2 ring-rose-200' : '';
  /** 错误和运行状态优先于选择态，其余状态仍允许选择态清晰可见。 */
  const surfaceTone = resolveSurfaceTone(status, selected);
  /** 选中卡片使用更明确的蓝色文字层级。 */
  const selectedTextTone = selected ? 'text-blue-950' : 'text-slate-800';
  const selectedDetailTone = selected ? 'text-blue-600' : 'text-slate-400';
  const Icon = NODE_ICONS[data.kind];

  return (
    <div
      className={`relative flex h-full w-full items-center gap-2 rounded-lg border pr-2.5 pl-3 ${surfaceTone} ${invalidTone}`}
      data-run-state={status}
      data-selected={selected ? 'true' : 'false'}
    >
      <span className={`absolute inset-y-0 left-0 w-1 ${tone.accent}`} aria-hidden="true" />
      <span className={`flex size-6 shrink-0 items-center justify-center rounded-md ${tone.icon}`}>
        <Icon className="size-3.5 stroke-[1.9]" aria-hidden="true" />
      </span>
      <div className={`flex min-w-0 flex-1 flex-col justify-center ${selectedTextTone}`}>
        <strong className="truncate text-[12px] leading-4 font-semibold">{data.label}</strong>
        <span className={`truncate text-[10px] leading-[14px] ${status === 'idle' ? selectedDetailTone : runtimeTone.status}`}>
          {status === 'idle' ? detail : runtimeTone.label}
        </span>
      </div>
      {StatusIcon ? (
        <StatusIcon
          aria-hidden="true"
          className={`size-4 shrink-0 stroke-[2.2] ${runtimeTone.status} ${status === 'running' ? 'animate-spin motion-reduce:animate-none' : ''}`}
        />
      ) : null}
    </div>
  );
}

/** 将节点专属配置压缩为卡片副标题。 */
function resolveNodeDetail(data: WorkflowNodeData): string {
  switch (data.kind) {
    case 'log':
      return data.message;
    case 'debug':
      return valueExprDetail(data.value);
    case 'delay':
      return `等待 ${data.milliseconds / 1000} 秒`;
    case 'condition':
      return `${valueExprDetail(data.left)} · ${data.operator}`;
    case 'variable':
      return `${data.assignments.length} 个变量赋值`;
    case 'application':
      return `${data.spec.acquire_policy} · ${data.spec.window_title.value}`;
    case 'browser':
      return data.spec.initial_url;
    case 'ui':
      return `${operationLabel(data.operation.type)} · ${locatorLabel(data.operation.target.locator.type)}`;
    case 'command':
      return `命令 · ${data.operation.runner}`;
    case 'start':
      return '手动触发';
    case 'end':
      return '流程结束';
  }
}

/** 解析运行态、选择态和默认态的卡片表面优先级。 */
function resolveSurfaceTone(status: NodeRunState, selected: boolean): string {
  if (status === 'error') {
    return 'border-rose-400 bg-rose-50/30 shadow-[0_3px_12px_rgba(225,29,72,.12)]';
  }
  if (status === 'running') {
    return [
      'border-blue-500 bg-white',
      'shadow-[0_0_0_3px_rgba(59,130,246,.10),0_6px_18px_rgba(37,99,235,.14)]',
      'after:pointer-events-none after:absolute after:-inset-[3px] after:rounded-[10px]',
      'after:border after:border-blue-400/50 after:animate-[argus-node-running_1.4s_ease-in-out_infinite]',
      'motion-reduce:after:animate-none',
    ].join(' ');
  }
  if (selected) {
    return 'border-blue-400 bg-blue-50 shadow-[0_4px_14px_rgba(37,99,235,0.14)]';
  }
  if (status === 'success') {
    return 'border-emerald-300 bg-white shadow-[0_3px_10px_rgba(16,185,129,.09)]';
  }
  if (status === 'skipped') {
    return 'border-slate-200 bg-slate-50/70 opacity-55 shadow-none';
  }
  return 'border-slate-200 bg-white shadow-[0_3px_10px_rgba(15,23,42,0.08)]';
}

/** 将调试值来源压缩为卡片可读文案。 */
function valueExprDetail(value: ValueExpr): string {
  switch (value.type) {
    case 'literal':
      if (typeof value.value === 'string') return value.value || '空字符串';
      return JSON.stringify(value.value);
    case 'expression':
      return value.source || '空表达式';
    case 'ref':
      return `${valueSourceDetail(value.source)}${value.pointer}`;
  }
}

/** 将结构化 ValueSource 压缩为卡片短标签。 */
function valueSourceDetail(source: Extract<ValueExpr, { type: 'ref' }>['source']): string {
  switch (source.type) {
    case 'workflow_input':
      return `输入 · ${source.key}`;
    case 'node':
      return source.node_id;
    case 'variable':
      return `变量 · ${source.name}`;
  }
}

/** 将目标定位类别压缩为卡片可读文案。 */
function locatorLabel(locator: TargetLocatorKind): string {
  switch (locator) {
    case 'query':
      return 'AQL';
    case 'visual':
      return '视觉文字';
    case 'coordinate':
      return '屏幕坐标';
  }
}

/** 将 UI 操作判别值转换成卡片短标签。 */
function operationLabel(operation: UiOperationKind): string {
  switch (operation) {
    case 'click': return '点击';
    case 'set_value': return '填写';
    case 'get_text': return '读取文本';
    case 'get_value': return '读取值';
    case 'collect_links': return '批量链接';
  }
}
