import {
  AppWindow,
  Bug,
  Braces,
  Boxes,
  Globe2,
  Navigation,
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
  TableProperties,
  Terminal,
  type LucideIcon,
} from 'lucide-react';

import type { FlowNodeRendererProps, NodeDefinition } from '../../../flow';
import type {
  AcquirePolicy,
  CommandRunner,
  ConditionOperator,
  UiOperationKind,
  ValueExpr,
} from '../../../features/workflow';
import {
  WORKFLOW_NODE_SIZES,
  type NodeRunState,
  type WorkflowNodeData,
} from '../../../features/workflow';

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
  navigate: Navigation,
  ui: MousePointerClick,
  command: Terminal,
  format: TableProperties,
  component: Boxes,
  end: Square,
};

/** 将配置中的稳定枚举转换成用户能直接理解的节点摘要。 */
const ACQUIRE_POLICY_LABELS: Readonly<Record<AcquirePolicy, string>> = {
  attach_or_start: '连接或打开',
  attach_only: '连接已有应用',
  always_start_new: '新开应用',
};

/** 命令运行方式的用户可见名称。 */
const COMMAND_RUNNER_LABELS: Readonly<Record<CommandRunner, string>> = {
  direct: '直接运行',
  power_shell: 'PowerShell',
  cmd: 'CMD',
};

/** 条件节点摘要使用与属性面板一致的中文运算符。 */
const CONDITION_OPERATOR_LABELS: Readonly<Record<ConditionOperator, string>> = {
  equal: '等于',
  not_equal: '不等于',
  greater_than: '大于',
  greater_than_or_equal: '大于等于',
  less_than: '小于',
  less_than_or_equal: '小于等于',
  contains: '包含',
  exists: '存在',
  not_exists: '不存在',
  is_empty: '为空',
  not_empty: '不为空',
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
  navigate: {
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
  format: {
    accent: 'bg-amber-500',
    icon: 'bg-amber-50 text-amber-700',
  },
  component: {
    accent: 'bg-violet-600',
    icon: 'bg-violet-50 text-violet-700',
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
  log: createDefinition('log', '记录日志', WORKFLOW_NODE_SIZES.log),
  debug: createDefinition('debug', '查看结果', WORKFLOW_NODE_SIZES.debug),
  delay: createDefinition('delay', '固定暂停', WORKFLOW_NODE_SIZES.delay),
  condition: createDefinition('condition', '条件判断', WORKFLOW_NODE_SIZES.condition),
  variable: createDefinition('variable', '设置变量', WORKFLOW_NODE_SIZES.variable),
  application: createDefinition('application', '打开应用', WORKFLOW_NODE_SIZES.application),
  browser: createDefinition('browser', '打开浏览器', WORKFLOW_NODE_SIZES.browser),
  navigate: createDefinition('navigate', '打开网页', WORKFLOW_NODE_SIZES.navigate),
  ui: createDefinition('ui', '操作界面', WORKFLOW_NODE_SIZES.ui),
  command: createDefinition('command', '执行命令', WORKFLOW_NODE_SIZES.command),
  format: createDefinition('format', '整理文本', WORKFLOW_NODE_SIZES.format),
  component: createDefinition('component', '流程组件', WORKFLOW_NODE_SIZES.component),
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
      return `暂停 ${data.milliseconds / 1000} 秒`;
    case 'condition':
      return `${valueExprDetail(data.left)} · ${CONDITION_OPERATOR_LABELS[data.operator]}`;
    case 'variable':
      return `设置 ${data.assignments.length} 个变量`;
    case 'application':
      return `${ACQUIRE_POLICY_LABELS[data.spec.acquire_policy]} · ${data.spec.window_title.value || '按程序查找窗口'}`;
    case 'browser':
      return '使用独立浏览器';
    case 'navigate':
      return valueExprDetail(data.operation.url);
    case 'ui':
      return operationLabel(data.operation.type);
    case 'command':
      return COMMAND_RUNNER_LABELS[data.operation.runner];
    case 'format':
      return `生成 ${data.operation.fields.length} 列文本`;
    case 'component':
      return `${data.componentName} · ${data.component.component_version}`;
    case 'start':
      return '手动运行';
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
      return '上游输出';
    case 'variable':
      return `变量 · ${source.name}`;
  }
}

/** 将 UI 操作判别值转换成卡片短标签。 */
function operationLabel(operation: UiOperationKind): string {
  switch (operation) {
    case 'click': return '点击';
    case 'set_value': return '输入文字';
    case 'press_key': return '按键';
    case 'type_text': return '物理输入文字';
    case 'get_text': return '读取文字';
    case 'get_value': return '读取控件值';
    case 'extract': return '读取数据';
    case 'collect_links': return '读取链接';
  }
}
