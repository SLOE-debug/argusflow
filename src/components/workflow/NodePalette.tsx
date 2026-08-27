import {
  ChevronRight,
  PanelLeft,
  Search,
  Workflow,
} from 'lucide-react';
import {
  useMemo,
  useState,
  type DragEvent as ReactDragEvent,
} from 'react';
import { useStore, type StoreApi } from 'zustand';

import {
  type FlowState,
  writeFlowNodeKindDragData,
} from '../../flow';
import type {
  WorkflowEdgeData,
  WorkflowNodeCreationKey,
  WorkflowNodeData,
} from '../../features/workflow/workflowModel';
import type { FlowComponentCatalogItem } from '../../features/workflow/componentCatalog';
import { FLOW_COMPONENT_CATALOG } from '../../features/workflow/componentCatalog';
import { Input } from '../ui';
import {
  PALETTE_GROUPS,
  PALETTE_ITEMS,
  type PaletteGroup,
  type PaletteItemDefinition,
} from './nodePaletteCatalog';
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
  /** 内置和当前工作区可创建的精确版本组件。 */
  componentCatalog?: ReadonlyArray<FlowComponentCatalogItem>;
}>;

/** 目录中已接入运行时、可以直接创建的节点定义。 */
type AvailablePaletteItem = PaletteItemDefinition & Readonly<{
  kind: WorkflowNodeCreationKey;
}>;

/** 可搜索的高密度分组节点库。 */
export function NodePalette({
  store,
  onResetWidth,
  componentCatalog = FLOW_COMPONENT_CATALOG,
}: NodePaletteProps) {
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
  /** 用户主动收起的节点分组；搜索不会隐式改变折叠偏好。 */
  const [collapsedGroups, setCollapsedGroups] = useState<ReadonlySet<PaletteGroup>>(
    () => new Set(),
  );
  const normalizedQuery = query.trim().toLocaleLowerCase();
  /** 工作区组件在创建后立即进入同一目录，不需要修改静态 Runtime type。 */
  const paletteItems = useMemo(() => {
    const knownKeys = new Set(PALETTE_ITEMS.flatMap((item) => item.kind ? [item.kind] : []));
    const workspaceComponents: PaletteItemDefinition[] = componentCatalog
      .filter((item) => !knownKeys.has(
        `component:${item.definition.id}@${item.definition.version}`,
      ))
      .map((item) => ({
        kind: `component:${item.definition.id}@${item.definition.version}`,
        title: item.title,
        description: item.description,
        group: 'component',
        icon: Workflow,
        iconClassName: 'bg-violet-50 text-violet-700',
      }));
    return [...PALETTE_ITEMS, ...workspaceComponents];
  }, [componentCatalog]);
  const visibleGroups = useMemo(() => PALETTE_GROUPS.flatMap((group) => {
    /** 当前分组中符合搜索词的条目。 */
    const matchingItems = paletteItems.filter((item) => (
      item.group === group.id
      && (
        item.title.toLocaleLowerCase().includes(normalizedQuery)
        || item.description.toLocaleLowerCase().includes(normalizedQuery)
      )
    ));
    /** 节点库只展示当前确实能够创建的节点，避免出现拖放禁止但没有解释的条目。 */
    const items = matchingItems.filter((item): item is AvailablePaletteItem => (
      isPaletteItemAvailable(item, startExists, endExists)
    ));
    return items.length > 0 ? [{ ...group, items }] : [];
  }), [endExists, normalizedQuery, paletteItems, startExists]);
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
    <aside className="relative z-10 flex h-full min-h-0 min-w-0 flex-col border-r border-slate-300 bg-slate-50">
      <div className="relative flex min-h-0 min-w-0 flex-1 flex-col">
        <header className="flex h-12 shrink-0 items-center border-b border-slate-200 bg-white px-3">
          <div className="min-w-0">
            <h2 className="truncate text-[13px] leading-4 font-semibold text-slate-900">
              {findPaletteModule(activeModule).label}
            </h2>
            {activeModule === 'nodes' ? (
              <p className="truncate text-[10px] leading-3.5 text-slate-400">
                拖拽节点到画布
              </p>
            ) : null}
          </div>
          {activeModule === 'nodes' ? (
            <button
              type="button"
              aria-label="恢复节点库默认宽度"
              className="ml-auto flex size-7 shrink-0 items-center justify-center border border-slate-200 text-slate-500 hover:bg-slate-100 hover:text-slate-900"
              onClick={onResetWidth}
              title="恢复默认宽度"
            >
              <PanelLeft className="size-3.5" aria-hidden="true" />
            </button>
          ) : null}
        </header>
        {activeModule === 'nodes' ? (
          <>
            <Input
              aria-label="搜索节点"
              density="compact"
              shape="square"
              containerClassName="mx-2.5 mt-2 shrink-0 bg-white"
              placeholder="搜索名称或用途"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              startAdornment={(
                <Search
                  className="size-3 shrink-0"
                  aria-hidden="true"
                />
              )}
            />
            <div className="mt-2 min-h-0 flex-1 overflow-y-auto px-2.5 pb-3">
              {visibleGroups.map((group) => (
                <section
                  key={group.id}
                  className="mb-3 last:mb-0"
                >
                  <button
                    type="button"
                    aria-expanded={!collapsedGroups.has(group.id)}
                    aria-controls={`palette-group-${group.id}`}
                    className="flex h-7 w-full items-center text-left text-slate-500 hover:text-slate-800"
                    onClick={() => toggleGroup(group.id)}
                  >
                    <ChevronRight
                      className={
                        'mr-1 size-3 shrink-0 transition-transform ' +
                        (collapsedGroups.has(group.id) ? '' : 'rotate-90')
                      }
                      aria-hidden="true"
                    />
                    <span className="shrink-0 text-[10px] font-semibold tracking-wide text-slate-600">
                      {group.label}
                    </span>
                    <span className="mx-2 h-px min-w-2 flex-1 bg-slate-200" aria-hidden="true" />
                    <span className="text-[9px] tabular-nums text-slate-400">
                      {group.items.length}
                    </span>
                  </button>
                  {!collapsedGroups.has(group.id) ? (
                    <div
                      id={`palette-group-${group.id}`}
                      className="grid grid-cols-2 gap-x-2"
                    >
                      {group.items.map((item) => (
                        <PaletteItemButton
                          key={item.title}
                          item={item}
                        />
                      ))}
                    </div>
                  ) : null}
                </section>
              ))}
              {visibleGroups.length === 0 ? (
                <div className="border border-dashed border-slate-300 px-3 py-8 text-center">
                  <p className="text-[11px] font-medium text-slate-600">没有匹配的节点</p>
                  <p className="mt-1 text-[10px] leading-4 text-slate-400">尝试搜索其他名称或用途</p>
                </div>
              ) : null}
            </div>
          </>
        ) : (
          <PaletteModulePlaceholder moduleId={activeModule} />
        )}
      </div>
      <PaletteNavigation
        activeModule={activeModule}
        onModuleChange={(module) => {
          setActiveModule(module);
        }}
      />
    </aside>
  );
}

/** 只保留已经实现且不与画布单例约束冲突的节点。 */
function isPaletteItemAvailable(
  item: PaletteItemDefinition,
  startExists: boolean,
  endExists: boolean,
): item is AvailablePaletteItem {
  if (!item.kind) return false;
  if (item.kind === 'start') return !startExists;
  if (item.kind === 'end') return !endExists;
  return true;
}

type PaletteItemButtonProps = Readonly<{
  /** 节点的稳定展示定义。 */
  item: AvailablePaletteItem;
}>;

/** 渲染整块可拖拽的横向节点图块。 */
function PaletteItemButton({ item }: PaletteItemButtonProps) {
  const Icon = item.icon;
  /** 原生拖放只传递节点注册键，实际创建与落点换算由画布负责。 */
  const handleDragStart = (event: ReactDragEvent<HTMLButtonElement>) => {
    event.dataTransfer.effectAllowed = 'copy';
    writeFlowNodeKindDragData(event.dataTransfer, item.kind);
  };

  return (
    <button
      type="button"
      aria-label={item.title}
      draggable
      onDragStart={handleDragStart}
      className={
        'group grid h-12 w-full cursor-grab select-none grid-cols-[28px_minmax(0,1fr)] ' +
        'items-center gap-2 rounded-md px-1.5 text-left ' +
        'hover:bg-white active:cursor-grabbing'
      }
    >
      <span
        className={`flex size-7 items-center justify-center rounded-md ${item.iconClassName}`}
      >
        <Icon className="size-4 stroke-[1.8]" aria-hidden="true" />
      </span>
      <span className="min-w-0">
        <strong
          className="block truncate text-[11px] leading-4 font-semibold text-slate-700"
          title={item.title}
        >
          {item.title}
        </strong>
        <span
          className="block truncate text-[9px] leading-3.5 text-slate-400"
          title={item.description}
        >
          {item.description}
        </span>
      </span>
    </button>
  );
}
