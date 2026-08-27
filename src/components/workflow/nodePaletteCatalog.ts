import {
  AlarmClock,
  AppWindow,
  Bell,
  Braces,
  Bug,
  Clock3,
  Globe2,
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

import type { EditableNodeKind } from '../../features/workflow/workflowModel';

/** 节点库中稳定且有序的业务分组。 */
export type PaletteGroup =
  | 'trigger'
  | 'control'
  | 'resource'
  | 'interface'
  | 'system'
  | 'data'
  | 'output';

/** 分组标题与用途说明。 */
export type PaletteGroupDefinition = Readonly<{
  id: PaletteGroup;
  label: string;
  description: string;
}>;

/** 节点目录条目的稳定展示契约。 */
export type PaletteItemDefinition = Readonly<{
  /** 后端已支持的节点类型；null 表示仅展示的后续节点。 */
  kind: EditableNodeKind | null;
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
    id: 'trigger',
    label: '触发',
    description: '决定流程何时开始',
  },
  {
    id: 'control',
    label: '流程控制',
    description: '改变执行顺序与路径',
  },
  {
    id: 'resource',
    label: '应用资源',
    description: '创建可复用的应用会话',
  },
  {
    id: 'interface',
    label: '界面操作',
    description: '定位并操作窗口控件',
  },
  {
    id: 'system',
    label: '系统',
    description: '调用本机系统能力',
  },
  {
    id: 'data',
    label: '数据处理',
    description: '整理流程中的数据',
  },
  {
    id: 'output',
    label: '输出与结束',
    description: '记录、发送或结束流程',
  },
] as const satisfies ReadonlyArray<PaletteGroupDefinition>;

/** 节点库条目的文案、用途与视觉身份。 */
export const PALETTE_ITEMS = [
  {
    kind: 'start',
    title: '手动触发',
    description: '由用户主动启动工作流',
    group: 'trigger',
    icon: PlayCircle,
    iconClassName: 'bg-emerald-50 text-emerald-700',
  },
  {
    kind: 'variable',
    title: '设置变量',
    description: '事务式更新运行变量',
    group: 'data',
    icon: Braces,
    iconClassName: 'bg-teal-50 text-teal-700',
  },
  {
    kind: null,
    title: '定时触发',
    description: '按预设时间自动启动',
    group: 'trigger',
    icon: AlarmClock,
    iconClassName: 'bg-emerald-50 text-emerald-700',
  },
  {
    kind: null,
    title: 'HTTP 触发',
    description: '收到网络请求时启动',
    group: 'trigger',
    icon: Webhook,
    iconClassName: 'bg-emerald-50 text-emerald-700',
  },
  {
    kind: null,
    title: '消息队列',
    description: '消费队列消息并启动',
    group: 'trigger',
    icon: MessageSquare,
    iconClassName: 'bg-emerald-50 text-emerald-700',
  },
  {
    kind: 'condition',
    title: '条件判断',
    description: '按表达式选择执行分支',
    group: 'control',
    icon: GitBranch,
    iconClassName: 'bg-violet-50 text-violet-700',
  },
  {
    kind: null,
    title: '并行处理',
    description: '同时执行多条路径',
    group: 'control',
    icon: Workflow,
    iconClassName: 'bg-violet-50 text-violet-700',
  },
  {
    kind: null,
    title: '循环处理',
    description: '重复执行一段流程',
    group: 'control',
    icon: Repeat2,
    iconClassName: 'bg-violet-50 text-violet-700',
  },
  {
    kind: 'delay',
    title: '延迟等待',
    description: '暂停指定的时间',
    group: 'control',
    icon: Clock3,
    iconClassName: 'bg-amber-50 text-amber-700',
  },
  {
    kind: 'application',
    title: '打开或连接应用',
    description: '创建桌面应用会话',
    group: 'resource',
    icon: AppWindow,
    iconClassName: 'bg-indigo-50 text-indigo-700',
  },
  {
    kind: 'browser',
    title: '打开浏览器',
    description: '创建隔离的 CDP 页面会话',
    group: 'resource',
    icon: Globe2,
    iconClassName: 'bg-sky-50 text-sky-700',
  },
  {
    kind: 'ui',
    title: '界面操作',
    description: '点击、填写或读取控件',
    group: 'interface',
    icon: MousePointerClick,
    iconClassName: 'bg-cyan-50 text-cyan-700',
  },
  {
    kind: 'command',
    title: '执行命令',
    description: '运行 PowerShell 或命令行',
    group: 'system',
    icon: Terminal,
    iconClassName: 'bg-slate-100 text-slate-700',
  },
  {
    kind: null,
    title: '脚本转换',
    description: '使用脚本转换输入数据',
    group: 'data',
    icon: FileCode2,
    iconClassName: 'bg-amber-50 text-amber-700',
  },
  {
    kind: null,
    title: '数据过滤',
    description: '保留满足条件的数据',
    group: 'data',
    icon: Filter,
    iconClassName: 'bg-amber-50 text-amber-700',
  },
  {
    kind: null,
    title: '数据聚合',
    description: '汇总多条输入数据',
    group: 'data',
    icon: Combine,
    iconClassName: 'bg-amber-50 text-amber-700',
  },
  {
    kind: null,
    title: '字段映射',
    description: '重组数据字段结构',
    group: 'data',
    icon: Shuffle,
    iconClassName: 'bg-amber-50 text-amber-700',
  },
  {
    kind: null,
    title: '写入数据库',
    description: '将结果持久化到数据库',
    group: 'output',
    icon: Database,
    iconClassName: 'bg-blue-50 text-blue-700',
  },
  {
    kind: null,
    title: '发送 HTTP',
    description: '把数据发送到外部服务',
    group: 'output',
    icon: Send,
    iconClassName: 'bg-blue-50 text-blue-700',
  },
  {
    kind: 'log',
    title: '写入日志',
    description: '记录流程运行信息',
    group: 'output',
    icon: FileText,
    iconClassName: 'bg-blue-50 text-blue-700',
  },
  {
    kind: 'debug',
    title: '调试输出',
    description: '检查运行中的数据值',
    group: 'output',
    icon: Bug,
    iconClassName: 'bg-fuchsia-50 text-fuchsia-700',
  },
  {
    kind: null,
    title: '消息通知',
    description: '向指定渠道发送通知',
    group: 'output',
    icon: Bell,
    iconClassName: 'bg-blue-50 text-blue-700',
  },
  {
    kind: 'end',
    title: '结束流程',
    description: '标记工作流正常完成',
    group: 'output',
    icon: Square,
    iconClassName: 'bg-rose-50 text-rose-700',
  },
] as const satisfies ReadonlyArray<PaletteItemDefinition>;
