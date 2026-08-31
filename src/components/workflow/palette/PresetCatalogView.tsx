import Search from 'lucide-react/dist/esm/icons/search.mjs';
import Sparkles from 'lucide-react/dist/esm/icons/sparkles.mjs';
import Workflow from 'lucide-react/dist/esm/icons/workflow.mjs';
import type { LucideIcon } from 'lucide-react';
import {
  useMemo,
  useState,
  type DragEvent as ReactDragEvent,
  type ReactNode,
} from 'react';

import { writeFlowNodeKindDragData } from '../../../flow';
import type {
  FlowComponentCatalogItem,
  WorkflowNodeCreationKey,
} from '../../../features/workflow';
import {
  FLOW_COMPONENT_CATALOG,
  NODE_PRESET_CATALOG,
} from '../../../features/workflow';
import { Button, Input } from '../../ui';

type PresetCatalogViewProps = Readonly<{
  /** 当前工作区内置和已创建的精确版本组件。 */
  componentCatalog?: ReadonlyArray<FlowComponentCatalogItem>;
}>;

/** 预设页将单节点预设和完整流程组件分成两类可拖拽资产。 */
export function PresetCatalogView({
  componentCatalog = FLOW_COMPONENT_CATALOG,
}: PresetCatalogViewProps) {
  const [query, setQuery] = useState('');
  const normalizedQuery = query.trim().toLocaleLowerCase();
  const visiblePresets = useMemo(
    () => NODE_PRESET_CATALOG.filter((preset) => matchesCatalogQuery(
      preset.title,
      preset.description,
      normalizedQuery,
    )),
    [normalizedQuery],
  );
  const visibleComponents = useMemo(
    () => componentCatalog.filter((component) => matchesCatalogQuery(
      component.title,
      component.description,
      normalizedQuery,
    )),
    [componentCatalog, normalizedQuery],
  );

  return (
    <div className="min-h-0 flex-1 overflow-y-auto px-2.5 pb-3">
      <Input
        aria-label="搜索预设"
        density="compact"
        shape="square"
        containerClassName="mt-2 bg-white"
        placeholder="搜索快捷操作或组合步骤"
        value={query}
        onChange={(event) => setQuery(event.target.value)}
        startAdornment={<Search className="size-3 shrink-0" aria-hidden="true" />}
      />
      <CatalogSection
        title="快捷操作"
        description="拖入画布后仍可继续修改"
        icon={Sparkles}
        emptyMessage="没有找到匹配的快捷操作"
      >
        {visiblePresets.map((preset) => (
          <PresetCard
            key={preset.id}
            title={preset.title}
            description={preset.description}
            icon={Sparkles}
            iconClassName="bg-cyan-50 text-cyan-700"
            creationKey={`preset:${preset.id}`}
          />
        ))}
      </CatalogSection>
      <CatalogSection
        title="组合步骤"
        description="一次加入一组已经编排好的步骤"
        icon={Workflow}
        emptyMessage="没有找到匹配的组合步骤"
      >
        {visibleComponents.map((component) => (
          <PresetCard
            key={`${component.definition.id}@${component.definition.version}`}
            title={component.title}
            description={`${component.description} · ${component.definition.version}`}
            icon={Workflow}
            iconClassName="bg-violet-50 text-violet-700"
            creationKey={`component:${component.definition.id}@${component.definition.version}`}
          />
        ))}
      </CatalogSection>
      {visiblePresets.length === 0 && visibleComponents.length === 0 ? (
        <div className="mt-3 border border-dashed border-slate-300 px-3 py-8 text-center">
          <p className="text-[11px] font-medium text-slate-600">找不到匹配的预设</p>
          <p className="mt-1 text-[10px] leading-4 text-slate-400">换个名称或用途试试</p>
        </div>
      ) : null}
    </div>
  );
}

type CatalogSectionProps = Readonly<{
  title: string;
  description: string;
  icon: LucideIcon;
  emptyMessage: string;
  children: ReactNode;
}>;

/** 统一预设页两类资产的标题和空状态。 */
function CatalogSection({
  title,
  description,
  icon: Icon,
  emptyMessage,
  children,
}: CatalogSectionProps) {
  const hasItems = Boolean(children && (Array.isArray(children) ? children.length : true));
  return (
    <section className="mt-4 first:mt-3">
      <div className="flex items-center gap-1.5">
        <Icon className="size-3.5 text-slate-500" aria-hidden="true" />
        <h3 className="text-[11px] font-semibold text-slate-700">{title}</h3>
      </div>
      <p className="mt-0.5 text-[10px] leading-4 text-slate-400">{description}</p>
      {hasItems ? (
        <div className="mt-1.5 flex flex-col gap-1">{children}</div>
      ) : (
        <p className="mt-2 rounded border border-dashed border-slate-200 px-2 py-3 text-center text-[10px] text-slate-400">
          {emptyMessage}
        </p>
      )}
    </section>
  );
}

type PresetCardProps = Readonly<{
  title: string;
  description: string;
  icon: LucideIcon;
  iconClassName: string;
  creationKey: WorkflowNodeCreationKey;
}>;

/** 预设和组件共用的拖拽卡片，只传递稳定创建键。 */
function PresetCard({
  title,
  description,
  icon: Icon,
  iconClassName,
  creationKey,
}: PresetCardProps) {
  const handleDragStart = (event: ReactDragEvent<HTMLButtonElement>) => {
    event.dataTransfer.effectAllowed = 'copy';
    writeFlowNodeKindDragData(event.dataTransfer, creationKey);
  };

  return (
    <Button
      type="button"
      variant="ghost"
      size="compact"
      aria-label={title}
      draggable
      onDragStart={handleDragStart}
      className="group grid h-auto min-h-12 w-full cursor-grab select-none grid-cols-[28px_minmax(0,1fr)] items-center gap-2 rounded-md border-transparent px-1.5 text-left whitespace-normal hover:bg-white active:cursor-grabbing"
    >
      <span className={`flex size-7 items-center justify-center rounded-md ${iconClassName}`}>
        <Icon className="size-4 shrink-0 stroke-[1.8]" aria-hidden="true" />
      </span>
      <span className="min-w-0">
        <strong className="block truncate text-[11px] leading-4 font-semibold text-slate-700" title={title}>
          {title}
        </strong>
        <span className="block truncate text-[9px] leading-3.5 text-slate-400" title={description}>
          {description}
        </span>
      </span>
    </Button>
  );
}

/** 预设文本搜索同时覆盖名称和用途说明。 */
function matchesCatalogQuery(title: string, description: string, query: string): boolean {
  if (!query) return true;
  return title.toLocaleLowerCase().includes(query)
    || description.toLocaleLowerCase().includes(query);
}
