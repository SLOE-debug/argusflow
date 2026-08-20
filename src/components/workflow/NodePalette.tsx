import { useMemo, useState } from 'react';
import {
  CirclePlay,
  Clock3,
  GitBranch,
  List,
  Search,
  Square,
  type LucideIcon,
} from 'lucide-react';

import type {
  EditableNodeKind,
  WorkflowCanvasNode,
} from '../../features/workflow/workflowModel';

type NodePaletteProps = {
  nodes: readonly WorkflowCanvasNode[];
  onAdd: (kind: EditableNodeKind) => void;
};

type PaletteGroup = '入口与出口' | '控制' | '输出';

type PaletteItem = {
  readonly kind: EditableNodeKind;
  readonly title: string;
  readonly description: string;
  readonly group: PaletteGroup;
  readonly icon: LucideIcon;
};

type PaletteItemButtonProps = {
  item: PaletteItem;
  disabled: boolean;
  onAdd: (kind: EditableNodeKind) => void;
};

/** 节点库条目的文案、分组与 Lucide 图标配置。 */
const items = [
  {
    kind: 'start',
    title: '开始',
    description: '工作流入口',
    group: '入口与出口',
    icon: CirclePlay,
  },
  {
    kind: 'end',
    title: '结束',
    description: '工作流出口',
    group: '入口与出口',
    icon: Square,
  },
  {
    kind: 'condition',
    title: '条件',
    description: '按 JSON 变量分支',
    group: '控制',
    icon: GitBranch,
  },
  {
    kind: 'delay',
    title: '等待',
    description: '暂停指定时长',
    group: '控制',
    icon: Clock3,
  },
  {
    kind: 'log',
    title: '日志',
    description: '写入运行事件',
    group: '输出',
    icon: List,
  },
] as const satisfies readonly PaletteItem[];

/** 节点类型对应的图标色与浅色底。 */
const itemTones: Record<EditableNodeKind, string> = {
  start: 'bg-emerald-100 text-emerald-700',
  end: 'bg-rose-100 text-rose-700',
  condition: 'bg-violet-100 text-violet-700',
  delay: 'bg-orange-100 text-orange-700',
  log: 'bg-blue-100 text-blue-700',
};

/** 可搜索的紧凑节点库；点击在默认位置新增节点。 */
export function NodePalette({ nodes, onAdd }: NodePaletteProps) {
  const [query, setQuery] = useState('');
  const normalizedQuery = query.trim().toLowerCase();
  const filteredItems = useMemo(
    () =>
      items.filter((item) =>
        `${item.title}${item.description}${item.group}`
          .toLowerCase()
          .includes(normalizedQuery),
      ),
    [normalizedQuery],
  );
  const groups = [...new Set(filteredItems.map((item) => item.group))];

  return (
    <aside
      className={
        'z-10 flex min-w-0 flex-col border-r border-slate-300/80 ' +
        'bg-slate-50 px-2.5 pt-3 pb-2'
      }
    >
      <div className="flex min-h-[34px] items-center">
        <div className="flex flex-col justify-center">
          <span className="text-[10px] leading-tight font-extrabold tracking-[.15em] text-blue-600">
            NODE LIBRARY
          </span>
          <h2 className="mt-0.5 text-base leading-tight font-bold">节点库</h2>
        </div>
      </div>
      <div
        className={
          'my-2 flex h-9 w-full items-center rounded-lg border border-slate-300 ' +
          'bg-white px-2.5 text-slate-500 shadow-[0_1px_2px_rgba(34,50,72,.03)] ' +
          'focus-within:border-blue-300 focus-within:ring-3 focus-within:ring-blue-100'
        }
      >
        <Search
          className="size-5 shrink-0"
          aria-hidden="true"
        />
        <input
          className={
            'h-full min-w-0 flex-1 border-0 bg-transparent pl-2 text-[13px] ' +
            'text-slate-800 outline-none placeholder:text-slate-400'
          }
          aria-label="搜索节点"
          placeholder="搜索节点…"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
        />
      </div>
      <div className="min-h-0 flex-1 overflow-auto">
        {groups.map((group) => (
          <section
            className="mb-3"
            key={group}
          >
            <h3 className="mb-1 ml-0.5 text-[11px] font-extrabold tracking-[.06em] text-slate-500">
              {group}
            </h3>
            {filteredItems
              .filter((item) => item.group === group)
              .map((item) => {
                const isSingleton = item.kind === 'start' || item.kind === 'end';
                const disabled =
                  isSingleton && nodes.some((node) => node.kind === item.kind);

                return (
                  <PaletteItemButton
                    key={item.kind}
                    item={item}
                    disabled={disabled}
                    onAdd={onAdd}
                  />
                );
              })}
          </section>
        ))}
      </div>
      <p className="mx-0.5 mt-1 text-[11px] leading-relaxed text-slate-500">
        <KeyboardHint>右键</KeyboardHint> 画布定位添加 · <KeyboardHint>Space</KeyboardHint>{' '}
        平移
      </p>
    </aside>
  );
}

/** 渲染单个可添加节点，并显示单例节点的禁用状态。 */
function PaletteItemButton({ item, disabled, onAdd }: PaletteItemButtonProps) {
  const Icon = item.icon;
  const addItem = () => onAdd(item.kind);

  return (
    <button
      type="button"
      disabled={disabled}
      onClick={addItem}
      className={
        'mb-0.5 flex min-h-[50px] w-full items-center gap-2 rounded-lg border ' +
        'border-transparent px-1.5 py-1 text-left text-slate-800 hover:border-slate-200 ' +
        'hover:bg-white hover:shadow-[0_4px_12px_rgba(38,57,82,.06)] ' +
        'disabled:cursor-not-allowed disabled:opacity-45'
      }
    >
      <span
        className={
          'flex size-[34px] shrink-0 items-center justify-center rounded-lg ' +
          itemTones[item.kind]
        }
      >
        <Icon
          className="size-5"
          aria-hidden="true"
        />
      </span>
      <span className="flex min-w-0 flex-1 flex-col justify-center">
        <strong className="text-sm leading-tight">{item.title}</strong>
        <small className="mt-0.5 truncate text-[11px] leading-tight text-slate-500">
          {disabled ? '已添加' : item.description}
        </small>
      </span>
    </button>
  );
}

/** 节点库操作提示中的统一键帽。 */
function KeyboardHint({ children }: { children: string }) {
  return (
    <kbd className="rounded border border-slate-300 bg-white px-1 py-px shadow-[0_1px_0_#d6deea]">
      {children}
    </kbd>
  );
}
