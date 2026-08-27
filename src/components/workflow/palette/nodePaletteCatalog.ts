import {
  AlarmClock,
  AppWindow,
  Bell,
  Braces,
  Bug,
  Clock3,
  Globe2,
  Navigation,
  TableProperties,
  Combine,
  Database,
  FileCode2,
  FileText,
  Filter,
  GitBranch,
  MessageSquare,
  MousePointerClick,
  PlayCircle,
  Repeat2,
  Send,
  Shuffle,
  Square,
  Terminal,
  Webhook,
  Workflow,
  type LucideIcon,
} from 'lucide-react';

import type { WorkflowNodeCreationKey } from '../../../features/workflow';
import { NODE_PRESET_CATALOG } from '../../../features/workflow';
import { FLOW_COMPONENT_CATALOG } from '../../../features/workflow';

/** 节点库中稳定且有序的业务分组。 */
export type PaletteGroup =
  | 'trigger'
  | 'control'
  | 'advanced'
  | 'resource'
  | 'interface'
  | 'system'
  | 'data'
  | 'output'
  | 'preset'
  | 'component';

/** 分组标题与用途说明。 */
export type PaletteGroupDefinition = Readonly<{
  id: PaletteGroup;
  label: string;
  description: string;
}>;

/** 节点目录条目的稳定展示契约。 */
export type PaletteItemDefinition = Readonly<{
  /** 后端已支持的节点类型；null 表示仅展示的后续节点。 */
  kind: WorkflowNodeCreationKey | null;
  /** 节点名称。 */
  title: string;
  /** 用一句动作描述说明节点进入流程后的作用。 */
  description: string;
  /** 所属分组。 */
  group: PaletteGroup;
  /** 与画布节点一致的语义图标。 */
  icon: LucideIcon;
  /** 节点图标色调。 */
  iconClassName: string;
}>;

/** 节点库的固定分组顺序；说明文本用于快速区分节点职责。 */
export const PALETTE_GROUPS = [
  {
    id: 'preset',
    label: '快捷操作',
    description: '一键添加常用操作',
  },
  {
    id: 'component',
    label: '可复用流程',
    description: '把常用流程重复使用',
  },
  {
    id: 'trigger',
    label: '触发流程',
    description: '选择流程何时开始',
  },
  {
    id: 'control',
    label: '流程分支',
    description: '决定下一步怎么走',
  },
  {
    id: 'advanced',
    label: '等待',
    description: '控制流程的运行节奏',
  },
  {
    id: 'resource',
    label: '应用和浏览器',
    description: '打开后续要用的应用',
  },
  {
    id: 'interface',
    label: '操作界面',
    description: '点击、输入或读取界面内容',
  },
  {
    id: 'system',
    label: '运行程序',
    description: '运行程序或系统命令',
  },
  {
    id: 'data',
    label: '处理数据',
    description: '整理流程中的数据',
  },
  {
    id: 'output',
    label: '输出结果',
    description: '保存结果或结束流程',
  },
] as const satisfies ReadonlyArray<PaletteGroupDefinition>;

/** 节点库条目的文案、用途与视觉身份。 */
export const PALETTE_ITEMS = [
  {
    kind: 'start',
    title: '开始',
    description: '手动开始运行',
    group: 'trigger',
    icon: PlayCircle,
    iconClassName: 'bg-emerald-50 text-emerald-700',
  },
  {
    kind: 'variable',
    title: '设置变量',
    description: '保存流程中要重复使用的数据',
    group: 'data',
    icon: Braces,
    iconClassName: 'bg-teal-50 text-teal-700',
  },
  {
    kind: null,
    title: '定时运行',
    description: '按设定时间自动开始',
    group: 'trigger',
    icon: AlarmClock,
    iconClassName: 'bg-emerald-50 text-emerald-700',
  },
  {
    kind: null,
    title: '收到请求',
    description: '收到请求后自动开始',
    group: 'trigger',
    icon: Webhook,
    iconClassName: 'bg-emerald-50 text-emerald-700',
  },
  {
    kind: null,
    title: '收到消息',
    description: '收到消息后自动开始',
    group: 'trigger',
    icon: MessageSquare,
    iconClassName: 'bg-emerald-50 text-emerald-700',
  },
  {
    kind: 'condition',
    title: '条件判断',
    description: '根据条件选择下一步怎么走',
    group: 'control',
    icon: GitBranch,
    iconClassName: 'bg-violet-50 text-violet-700',
  },
  {
    kind: null,
    title: '并行执行',
    description: '同时运行多条路径',
    group: 'control',
    icon: Workflow,
    iconClassName: 'bg-violet-50 text-violet-700',
  },
  {
    kind: null,
    title: '循环执行',
    description: '重复运行一段流程',
    group: 'control',
    icon: Repeat2,
    iconClassName: 'bg-violet-50 text-violet-700',
  },
  {
    kind: 'delay',
    title: '等待',
    description: '等一段时间再继续',
    group: 'advanced',
    icon: Clock3,
    iconClassName: 'bg-amber-50 text-amber-700',
  },
  {
    kind: 'application',
    title: '打开应用',
    description: '打开或连接已有应用',
    group: 'resource',
    icon: AppWindow,
    iconClassName: 'bg-indigo-50 text-indigo-700',
  },
  {
    kind: 'browser',
    title: '打开浏览器',
    description: '打开一个独立浏览器',
    group: 'resource',
    icon: Globe2,
    iconClassName: 'bg-sky-50 text-sky-700',
  },
  {
    kind: 'navigate',
    title: '打开网页',
    description: '在浏览器中打开网址',
    group: 'resource',
    icon: Navigation,
    iconClassName: 'bg-sky-50 text-sky-700',
  },
  {
    kind: 'format',
    title: '整理文本',
    description: '把数据整理成可读文本',
    group: 'data',
    icon: TableProperties,
    iconClassName: 'bg-amber-50 text-amber-700',
  },
  {
    kind: 'ui',
    title: '操作界面',
    description: '点击、输入或读取界面内容',
    group: 'interface',
    icon: MousePointerClick,
    iconClassName: 'bg-cyan-50 text-cyan-700',
  },
  {
    kind: 'command',
    title: '执行命令',
    description: '运行程序或系统命令',
    group: 'system',
    icon: Terminal,
    iconClassName: 'bg-slate-100 text-slate-700',
  },
  {
    kind: null,
    title: '脚本处理',
    description: '用脚本处理数据',
    group: 'data',
    icon: FileCode2,
    iconClassName: 'bg-amber-50 text-amber-700',
  },
  {
    kind: null,
    title: '筛选数据',
    description: '只保留符合条件的数据',
    group: 'data',
    icon: Filter,
    iconClassName: 'bg-amber-50 text-amber-700',
  },
  {
    kind: null,
    title: '合并数据',
    description: '合并多条数据',
    group: 'data',
    icon: Combine,
    iconClassName: 'bg-amber-50 text-amber-700',
  },
  {
    kind: null,
    title: '整理字段',
    description: '调整数据字段',
    group: 'data',
    icon: Shuffle,
    iconClassName: 'bg-amber-50 text-amber-700',
  },
  {
    kind: null,
    title: '写入数据库',
    description: '把结果保存到数据库',
    group: 'output',
    icon: Database,
    iconClassName: 'bg-blue-50 text-blue-700',
  },
  {
    kind: null,
    title: '发送 HTTP 请求',
    description: '把数据发送到外部服务',
    group: 'output',
    icon: Send,
    iconClassName: 'bg-blue-50 text-blue-700',
  },
  {
    kind: 'log',
    title: '记录日志',
    description: '记录流程运行信息',
    group: 'output',
    icon: FileText,
    iconClassName: 'bg-blue-50 text-blue-700',
  },
  {
    kind: 'debug',
    title: '查看结果',
    description: '查看节点运行结果',
    group: 'output',
    icon: Bug,
    iconClassName: 'bg-fuchsia-50 text-fuchsia-700',
  },
  {
    kind: null,
    title: '发送通知',
    description: '向指定渠道发送消息',
    group: 'output',
    icon: Bell,
    iconClassName: 'bg-blue-50 text-blue-700',
  },
  {
    kind: 'end',
    title: '结束',
    description: '在这里结束流程',
    group: 'output',
    icon: Square,
    iconClassName: 'bg-rose-50 text-rose-700',
  },
  ...NODE_PRESET_CATALOG.map((preset) => ({
    kind: `preset:${preset.id}` as const,
    title: preset.title,
    description: preset.description,
    group: 'preset' as const,
    icon: MousePointerClick,
    iconClassName: 'bg-cyan-50 text-cyan-700',
  })),
  ...FLOW_COMPONENT_CATALOG.map((item) => ({
    kind: `component:${item.definition.id}@${item.definition.version}` as const,
    title: item.title,
    description: item.description,
    group: 'component' as const,
    icon: Workflow,
    iconClassName: 'bg-violet-50 text-violet-700',
  })),
] satisfies ReadonlyArray<PaletteItemDefinition>;
