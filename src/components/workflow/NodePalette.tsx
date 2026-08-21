import {
  AlarmClock,
  Bell,
  ChevronRight,
  Clock3,
  Combine,
  Database,
  FileCode2,
  FileText,
  Filter,
  GitBranch,
  MessageSquare,
  MousePointer2,
  PanelLeft,
  Repeat2,
  Search,
  Send,
  Shuffle,
  SlidersHorizontal,
  Square,
  Webhook,
  Workflow,
  type LucideIcon,
} from 'lucide-react';
import {
  useMemo,
  useState,
  type DragEvent as ReactDragEvent,
} from 'react';
import { useStore, type StoreApi } from 'zustand';

import {
  FLOW_NODE_KIND_DRAG_TYPE,
  type FlowState,
} from '../../flow';
import type {
  EditableNodeKind,
  WorkflowEdgeData,
  WorkflowNodeData,
} from '../../features/workflow/workflowModel';
import { Input } from '../ui';
import {
  findPaletteModule,
  PaletteModulePlaceholder,
  PaletteNavigation,
  type PaletteModule,
} from './PaletteNavigation';

type NodePaletteProps = Readonly<{
  /** 画布 Store；节点库仅订阅两个单例节点是否存在。 */
  store: StoreApi<FlowState<WorkflowNodeData, WorkflowEdgeData>>;
  /** 恢复左侧面板的默认宽度。 */
  onResetWidth: () => void;
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

/** 各节点分组的轻量图标色调。 */
const PALETTE_GROUP_TONES = {
  input: 'bg-emerald-50 text-emerald-600',
  control: 'bg-violet-50 text-violet-600',
  data: 'bg-amber-50 text-amber-600',
  output: 'bg-blue-50 text-blue-600',
} as const satisfies Readonly<Record<PaletteGroup, string>>;

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

/** 可搜索的高密度分组节点库。 */
export function NodePalette({ store, onResetWidth }: NodePaletteProps) {
  const startExists = useStore(
    store,
    (state) => state.nodes.some((node) => node.kind === 'start'),
  );
  const endExists = useStore(
    store,
    (state) => state.nodes.some((node) => node.kind === 'end'),
  );
  const [query, setQuery] = useState('');
  const [activeModule, setActiveModule] = useState<PaletteModule>('nodes');
  const [filterOpen, setFilterOpen] = useState(false);
  const [onlyAvailable, setOnlyAvailable] = useState(false);
  /** 用户主动收起的节点分组；搜索不会隐式改变折叠偏好。 */
  const [collapsedGroups, setCollapsedGroups] = useState<ReadonlySet<PaletteGroup>>(
    () => new Set(),
  );
  const normalizedQuery = query.trim().toLocaleLowerCase();
  const visibleGroups = useMemo(() => PALETTE_GROUPS.flatMap((group) => {
    /** 当前分组中符合搜索词的条目。 */
    const items = PALETTE_ITEMS.filter((item) => (
      item.group === group.id
      && item.title.toLocaleLowerCase().includes(normalizedQuery)
      && (!onlyAvailable || isPaletteItemAvailable(item, startExists, endExists))
    ));
    return items.length > 0 ? [{ ...group, items }] : [];
  }), [endExists, normalizedQuery, onlyAvailable, startExists]);
  /** 切换单个分组时复制集合，保持 React 状态不可变。 */
  const toggleGroup = (groupId: PaletteGroup) => {
    setCollapsedGroups((current) => {
      const next = new Set(current);
      if (next.has(groupId)) next.delete(groupId);
      else next.add(groupId);
      return next;
    });
  };

  return (
    <aside className="relative z-10 flex h-full min-h-0 min-w-0 flex-col border-r border-slate-200 bg-white">
      <header className="flex h-[34px] shrink-0 items-center border-b border-slate-100 px-2.5">
        <h2 className="truncate text-[12px] leading-none font-semibold text-slate-800">
          {findPaletteModule(activeModule).label}
        </h2>
        {activeModule === 'nodes' ? (
          <>
            <button
              type="button"
              aria-label="恢复节点库默认宽度"
              className="ml-auto flex size-7 items-center justify-center rounded-md text-slate-500 hover:bg-slate-100 hover:text-slate-800"
              onClick={onResetWidth}
              title="恢复默认宽度"
            >
              <PanelLeft className="size-3.5" aria-hidden="true" />
            </button>
            <button
              type="button"
              aria-label="节点库筛选"
              aria-expanded={filterOpen}
              className={`ml-0.5 flex size-7 items-center justify-center rounded-md hover:bg-slate-100 hover:text-slate-800 ${filterOpen || onlyAvailable ? 'bg-blue-50 text-blue-600' : 'text-slate-500'}`}
              onClick={() => setFilterOpen((open) => !open)}
            >
              <SlidersHorizontal className="size-3.5" aria-hidden="true" />
            </button>
          </>
        ) : null}
      </header>
      {activeModule === 'nodes' && filterOpen ? (
        <PaletteFilterPanel
          onlyAvailable={onlyAvailable}
          onOnlyAvailableChange={setOnlyAvailable}
          onExpandAll={() => setCollapsedGroups(new Set())}
          onCollapseAll={() => setCollapsedGroups(new Set(
            PALETTE_GROUPS.map((group) => group.id),
          ))}
        />
      ) : null}
      {activeModule === 'nodes' ? (
        <>
          <Input
            aria-label="搜索节点"
            density="compact"
            containerClassName="mx-2.5 mt-2 shrink-0"
            placeholder="搜索节点"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            startAdornment={(
              <Search
                className="size-3 shrink-0"
                aria-hidden="true"
              />
            )}
          />
          <div className="mt-2 min-h-0 flex-1 overflow-y-auto px-2 pb-2">
            {visibleGroups.map((group) => (
              <section
                key={group.id}
                className="mb-2 border-b border-slate-100 pb-2 last:mb-0 last:border-b-0"
              >
                <button
                  type="button"
                  aria-expanded={!collapsedGroups.has(group.id)}
                  aria-controls={`palette-group-${group.id}`}
                  className="flex h-7 w-full items-center rounded-md px-1 text-[12px] leading-none font-semibold text-slate-600 hover:bg-slate-50 hover:text-slate-800"
                  onClick={() => toggleGroup(group.id)}
                >
                  <ChevronRight
                    className={
                      'mr-1 size-2.5 transition-transform ' +
                      (collapsedGroups.has(group.id) ? '' : 'rotate-90')
                    }
                    aria-hidden="true"
                  />
                  {group.label}
                  <span className="ml-auto text-[10px] font-normal text-slate-400">
                    {group.items.length}
                  </span>
                </button>
                {!collapsedGroups.has(group.id) ? (
                  <div
                    id={`palette-group-${group.id}`}
                    className="mt-1 grid grid-cols-2 gap-1.5 px-0.5"
                  >
                    {group.items.map((item) => (
                    <PaletteItemButton
                      key={item.title}
                      item={item}
                      disabled={!isPaletteItemAvailable(item, startExists, endExists)}
                    />
                    ))}
                  </div>
                ) : null}
              </section>
            ))}
          </div>
        </>
      ) : (
        <PaletteModulePlaceholder moduleId={activeModule} />
      )}
      <PaletteNavigation
        activeModule={activeModule}
        onModuleChange={(module) => {
          setActiveModule(module);
          setFilterOpen(false);
        }}
      />
    </aside>
  );
}

/** 判断节点条目是否已有实现且不与单例节点冲突。 */
function isPaletteItemAvailable(
  item: PaletteItem,
  startExists: boolean,
  endExists: boolean,
): boolean {
  if (!item.kind) return false;
  if (item.kind === 'start') return !startExists;
  if (item.kind === 'end') return !endExists;
  return true;
}

type PaletteFilterPanelProps = Readonly<{
  /** 是否只显示已可用节点。 */
  onlyAvailable: boolean;
  /** 更新可用节点筛选。 */
  onOnlyAvailableChange: (checked: boolean) => void;
  /** 展开所有节点分组。 */
  onExpandAll: () => void;
  /** 收起所有节点分组。 */
  onCollapseAll: () => void;
}>;

/** 节点库头部筛选按钮对应的轻量功能面板。 */
function PaletteFilterPanel({
  onlyAvailable,
  onOnlyAvailableChange,
  onExpandAll,
  onCollapseAll,
}: PaletteFilterPanelProps) {
  return (
    <div className="absolute top-[32px] right-2 z-40 w-44 rounded-md border border-slate-200 bg-white p-2 shadow-lg">
      <label className="flex h-7 items-center gap-2 rounded px-1 text-[11px] text-slate-700 hover:bg-slate-50">
        <input
          type="checkbox"
          checked={onlyAvailable}
          onChange={(event) => onOnlyAvailableChange(event.target.checked)}
          className="size-3.5 accent-blue-600"
        />
        仅显示可用节点
      </label>
      <div className="mt-1 grid grid-cols-2 gap-1 border-t border-slate-100 pt-2">
        <button
          type="button"
          className="h-7 rounded bg-slate-50 text-[10px] text-slate-600 hover:bg-slate-100"
          onClick={onExpandAll}
        >
          全部展开
        </button>
        <button
          type="button"
          className="h-7 rounded bg-slate-50 text-[10px] text-slate-600 hover:bg-slate-100"
          onClick={onCollapseAll}
        >
          全部收起
        </button>
      </div>
    </div>
  );
}

type PaletteItemButtonProps = Readonly<{
  item: PaletteItem;
  disabled: boolean;
}>;

/** 渲染可拖入画布的紧凑节点磁贴。 */
function PaletteItemButton({ item, disabled }: PaletteItemButtonProps) {
  const Icon = item.icon;
  /** 原生拖放只传递节点注册键，实际创建与落点换算由画布负责。 */
  const handleDragStart = (event: ReactDragEvent<HTMLButtonElement>) => {
    if (!item.kind || disabled) {
      event.preventDefault();
      return;
    }
    event.dataTransfer.effectAllowed = 'copy';
    event.dataTransfer.setData(FLOW_NODE_KIND_DRAG_TYPE, item.kind);
  };

  return (
    <button
      type="button"
      disabled={disabled}
      draggable={!disabled}
      onDragStart={handleDragStart}
      className="group flex h-10 w-full cursor-grab items-center rounded-lg border border-slate-200 bg-white px-2 text-left text-[12px] leading-none text-slate-700 shadow-[0_1px_2px_rgba(15,23,42,0.03)] hover:border-blue-300 hover:shadow-[0_3px_8px_rgba(37,99,235,0.08)] active:cursor-grabbing disabled:pointer-events-none disabled:cursor-default disabled:opacity-40"
    >
      <span className={`flex size-5 shrink-0 items-center justify-center rounded-md ${PALETTE_GROUP_TONES[item.group]}`}>
        <Icon className="size-3 stroke-[1.8]" aria-hidden="true" />
      </span>
      <span className="ml-2 flex-1 truncate">{item.title}</span>
    </button>
  );
}
