import AppWindow from 'lucide-react/dist/esm/icons/app-window.mjs';
import Braces from 'lucide-react/dist/esm/icons/braces.mjs';
import Bug from 'lucide-react/dist/esm/icons/bug.mjs';
import Clock3 from 'lucide-react/dist/esm/icons/clock-3.mjs';
import Combine from 'lucide-react/dist/esm/icons/combine.mjs';
import FileCode2 from 'lucide-react/dist/esm/icons/file-code-corner.mjs';
import FileText from 'lucide-react/dist/esm/icons/file-text.mjs';
import Filter from 'lucide-react/dist/esm/icons/funnel.mjs';
import GitBranch from 'lucide-react/dist/esm/icons/git-branch.mjs';
import Globe2 from 'lucide-react/dist/esm/icons/earth.mjs';
import Navigation from 'lucide-react/dist/esm/icons/navigation.mjs';
import MousePointerClick from 'lucide-react/dist/esm/icons/mouse-pointer-click.mjs';
import PlayCircle from 'lucide-react/dist/esm/icons/circle-play.mjs';
import Repeat2 from 'lucide-react/dist/esm/icons/repeat-2.mjs';
import Shuffle from 'lucide-react/dist/esm/icons/shuffle.mjs';
import Square from 'lucide-react/dist/esm/icons/square.mjs';
import TableProperties from 'lucide-react/dist/esm/icons/table-properties.mjs';
import Terminal from 'lucide-react/dist/esm/icons/terminal.mjs';
import Workflow from 'lucide-react/dist/esm/icons/workflow.mjs';
import type { LucideIcon } from 'lucide-react';

import type { WorkflowNodeCreationKey } from '../../../features/workflow';

/** 节点页中稳定且有序的五个业务分组。 */
export type PaletteGroup =
  | 'control'
  | 'resource'
  | 'interface'
  | 'system'
  | 'data';

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

/** 节点库的固定分组顺序；节点页不混入预设或组件。 */
export const PALETTE_GROUPS = [
  {
    id: 'control',
    label: '流程控制',
    description: '决定流程如何开始、暂停、分支和结束',
  },
  {
    id: 'resource',
    label: '资源',
    description: '打开后续操作要使用的应用或浏览器',
  },
  {
    id: 'interface',
    label: '界面与浏览器',
    description: '与桌面界面或浏览器页面交互',
  },
  {
    id: 'system',
    label: '系统',
    description: '运行程序或系统命令',
  },
  {
    id: 'data',
    label: '数据与输出',
    description: '保存、整理、记录和查看流程数据',
  },
] as const satisfies ReadonlyArray<PaletteGroupDefinition>;

/** 节点页的文案、用途与视觉身份。 */
export const PALETTE_ITEMS = [
  {
    kind: 'start',
    title: '开始',
    description: '手动开始运行',
    group: 'control',
    icon: PlayCircle,
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
    kind: 'delay',
    title: '固定暂停',
    description: '暂停固定时长，不检测目标是否就绪',
    group: 'control',
    icon: Clock3,
    iconClassName: 'bg-amber-50 text-amber-700',
  },
  {
    kind: 'end',
    title: '结束',
    description: '在这里结束流程',
    group: 'control',
    icon: Square,
    iconClassName: 'bg-rose-50 text-rose-700',
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
    kind: 'ui',
    title: '操作界面',
    description: '点击、输入或读取界面内容',
    group: 'interface',
    icon: MousePointerClick,
    iconClassName: 'bg-cyan-50 text-cyan-700',
  },
  {
    kind: 'navigate',
    title: '打开网页',
    description: '在浏览器中打开网址',
    group: 'interface',
    icon: Navigation,
    iconClassName: 'bg-sky-50 text-sky-700',
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
    kind: 'variable',
    title: '设置变量',
    description: '保存流程中要重复使用的数据',
    group: 'data',
    icon: Braces,
    iconClassName: 'bg-teal-50 text-teal-700',
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
    kind: 'log',
    title: '记录日志',
    description: '记录流程运行信息',
    group: 'data',
    icon: FileText,
    iconClassName: 'bg-blue-50 text-blue-700',
  },
  {
    kind: 'debug',
    title: '查看结果',
    description: '查看节点运行结果',
    group: 'data',
    icon: Bug,
    iconClassName: 'bg-fuchsia-50 text-fuchsia-700',
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
] satisfies ReadonlyArray<PaletteItemDefinition>;
