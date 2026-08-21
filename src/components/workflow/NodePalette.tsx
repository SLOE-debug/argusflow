import {
  AlarmClock,
  Bell,
  Boxes,
  Clock3,
  Combine,
  Database,
  FileCode2,
  FileText,
  Filter,
  GitBranch,
  GripVertical,
  Layers3,
  MessageSquare,
  MousePointer2,
  PanelLeft,
  Pin,
  Repeat2,
  Search,
  Send,
  Settings,
  Shuffle,
  SlidersHorizontal,
  Square,
  Webhook,
  Workflow,
  type LucideIcon,
} from 'lucide-react';
import { useMemo, useState } from 'react';

import type {
  EditableNodeKind,
  WorkflowCanvasNode,
} from '../../features/workflow/workflowModel';

type NodePaletteProps = Readonly<{
  /** 当前画布节点，用于判断单例节点是否已经存在。 */
  nodes: ReadonlyArray<WorkflowCanvasNode>;
  /** 添加后端已支持的节点类型。 */
  onAdd: (kind: EditableNodeKind) => void;
}>;

type PaletteGroup = 'input' | 'control' | 'data' | 'output';

type PaletteItem = Readonly<{
  /** 后端已支持的节点类型；null 表示仅展示的后续节点。 */
  kind: EditableNodeKind | null;
  /** 节点名称。 */
  title: string;
  /** 所属分组。 */
  group: PaletteGroup;
  /** 清晰的 Lucide 替代图标。 */
  icon: LucideIcon;
}>;

/** 参考图左侧节点库的分组顺序与标题。 */
const PALETTE_GROUPS = [
  { id: 'input', label: '输入' },
  { id: 'control', label: '流程控制' },
  { id: 'data', label: '数据处理' },
  { id: 'output', label: '输出' },
] as const satisfies ReadonlyArray<Readonly<{ id: PaletteGroup; label: string }>>;

/** 节点库条目的文案、分组与 Lucide 图标配置。 */
const PALETTE_ITEMS = [
  { kind: 'start', title: '手动触发', group: 'input', icon: MousePointer2 },
  { kind: null, title: '定时触发', group: 'input', icon: AlarmClock },
  { kind: null, title: 'HTTP 触发', group: 'input', icon: Webhook },
  { kind: null, title: '消息队列', group: 'input', icon: MessageSquare },
  { kind: 'condition', title: '条件判断', group: 'control', icon: GitBranch },
  { kind: null, title: '并行处理', group: 'control', icon: Workflow },
  { kind: null, title: '循环处理', group: 'control', icon: Repeat2 },
  { kind: 'delay', title: '延迟等待', group: 'control', icon: Clock3 },
  { kind: null, title: '脚本转换', group: 'data', icon: FileCode2 },
  { kind: null, title: '数据过滤', group: 'data', icon: Filter },
  { kind: null, title: '数据聚合', group: 'data', icon: Combine },
  { kind: null, title: '字段映射', group: 'data', icon: Shuffle },
  { kind: null, title: '写入数据库', group: 'output', icon: Database },
  { kind: null, title: '发送 HTTP', group: 'output', icon: Send },
  { kind: 'log', title: '写入日志', group: 'output', icon: FileText },
  { kind: null, title: '消息通知', group: 'output', icon: Bell },
  { kind: 'end', title: '结束流程', group: 'output', icon: Square },
] as const satisfies ReadonlyArray<PaletteItem>;

/** 可搜索的参考图高密度节点库。 */
export function NodePalette({ nodes, onAdd }: NodePaletteProps) {
  const [query, setQuery] = useState('');
  const normalizedQuery = query.trim().toLocaleLowerCase();
  const visibleGroups = useMemo(() => PALETTE_GROUPS.flatMap((group) => {
    /** 当前分组中符合搜索词的条目。 */
    const items = PALETTE_ITEMS.filter((item) => (
      item.group === group.id && item.title.toLocaleLowerCase().includes(normalizedQuery)
    ));
    return items.length > 0 ? [{ ...group, items }] : [];
  }), [normalizedQuery]);

  return (
    <aside className="z-10 flex min-h-0 min-w-0 flex-col border-r border-slate-200 bg-slate-50">
      <header className="flex h-[42px] shrink-0 items-center px-3">
        <h2 className="text-[15px] font-semibold text-slate-800">节点库</h2>
        <button type="button" aria-label="固定节点库" className="ml-auto text-slate-500">
          <Pin className="size-4" aria-hidden="true" />
        </button>
        <button type="button" aria-label="节点库筛选" className="ml-4 text-slate-500">
          <SlidersHorizontal className="size-4" aria-hidden="true" />
        </button>
      </header>
      <label className="mx-3 flex h-9 shrink-0 items-center rounded-md border border-slate-300 bg-white px-2.5 text-slate-400 focus-within:border-blue-400 focus-within:ring-1 focus-within:ring-blue-100">
        <Search className="size-4 shrink-0" aria-hidden="true" />
        <input
          className="h-full min-w-0 flex-1 border-0 bg-transparent pl-2 text-[12px] text-slate-800 outline-none placeholder:text-slate-400"
          aria-label="搜索节点"
          placeholder="搜索节点"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
        />
      </label>
      <div className="mt-2 min-h-0 flex-1 overflow-y-auto px-2.5 pb-2">
        {visibleGroups.map((group) => (
          <section key={group.id} className="mb-2">
            <h3 className="flex h-6 items-center text-[11px] font-semibold text-slate-700">
              <span className="mr-1 text-[9px] text-slate-500">⌄</span>
              {group.label}
            </h3>
            <div className="overflow-hidden rounded-md border border-slate-200 bg-white">
              {group.items.map((item) => {
                const isSingleton = item.kind === 'start' || item.kind === 'end';
                const exists = isSingleton && nodes.some((node) => node.kind === item.kind);
                return (
                  <PaletteItemButton
                    key={item.title}
                    item={item}
                    disabled={item.kind === null || exists}
                    onAdd={onAdd}
                  />
                );
              })}
            </div>
          </section>
        ))}
      </div>
      <PaletteNavigation />
    </aside>
  );
}

type PaletteItemButtonProps = Readonly<{
  item: PaletteItem;
  disabled: boolean;
  onAdd: (kind: EditableNodeKind) => void;
}>;

/** 渲染单个 31px 节点条目；未实现条目保持可见但不可添加。 */
function PaletteItemButton({ item, disabled, onAdd }: PaletteItemButtonProps) {
  const Icon = item.icon;
  const addItem = () => {
    if (item.kind) onAdd(item.kind);
  };

  return (
    <button
      type="button"
      disabled={disabled}
      onClick={addItem}
      className="group flex h-[31px] w-full items-center border-b border-slate-100 px-2 text-left text-[12px] text-slate-700 last:border-b-0 hover:bg-blue-50 disabled:opacity-100"
    >
      <Icon className="size-4 shrink-0 stroke-[1.7] text-slate-600" aria-hidden="true" />
      <span className="ml-2 flex-1 truncate">{item.title}</span>
      <GripVertical className="size-3.5 text-slate-300 group-hover:text-slate-400" aria-hidden="true" />
    </button>
  );
}

/** 节点库底部的五项模块导航。 */
function PaletteNavigation() {
  const navigation = [Layers3, PanelLeft, Boxes, Workflow, Settings] as const;
  return (
    <nav aria-label="工作台模块" className="flex h-11 shrink-0 items-center justify-around border-t border-slate-200 bg-white">
      {navigation.map((Icon, index) => (
        <button
          key={Icon.displayName ?? index}
          type="button"
          aria-label={`工作台模块 ${index + 1}`}
          className={
            'relative flex h-11 flex-1 items-center justify-center ' +
            (index === 0
              ? 'text-blue-600 after:absolute after:bottom-0 after:h-0.5 after:w-8 after:bg-blue-600'
              : 'text-slate-500 hover:text-slate-800')
          }
        >
          <Icon className="size-4" aria-hidden="true" />
        </button>
      ))}
    </nav>
  );
}
