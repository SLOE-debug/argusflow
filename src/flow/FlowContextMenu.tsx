import { useState } from 'react';
import {
  AlignCenterHorizontal,
  AlignCenterVertical,
  AlignEndHorizontal,
  AlignEndVertical,
  AlignHorizontalSpaceBetween,
  AlignStartHorizontal,
  AlignStartVertical,
  AlignVerticalSpaceBetween,
  ChevronRight,
  CirclePlay,
  Clock3,
  GitBranch,
  LayoutPanelLeft,
  List,
  Plus,
  Square,
  type LucideIcon,
} from 'lucide-react';

import type { AlignMode, DistributeMode } from './selection';
import { useFlowStore } from './store';
import type { FlowNode, NodeDefinition, NodeRegistry } from './types';
import type { CanvasContextMenu } from './useCanvasPointerInteractions';

type FlowContextMenuProps = Readonly<{
  /** 菜单位置、添加坐标与二级菜单展开方向。 */
  context: CanvasContextMenu;
  /** 业务注册的可添加节点。 */
  registry: Readonly<NodeRegistry>;
  /** 当前节点用于判断单例节点是否已经存在。 */
  nodes: ReadonlyArray<FlowNode>;
  /** 在右键位置添加节点。 */
  onAddNode: (kind: string, position: CanvasContextMenu['world']) => void;
  /** 任一菜单操作完成后关闭菜单。 */
  onClose: () => void;
}>;

type ArrangeAction = Readonly<{
  icon: LucideIcon;
  label: string;
}> & (
  | Readonly<{ kind: 'align'; mode: AlignMode }>
  | Readonly<{ kind: 'distribute'; mode: DistributeMode }>
);

/** 右键菜单内业务节点类型对应的图标。 */
const NODE_ICONS: Readonly<Record<string, LucideIcon>> = {
  start: CirclePlay,
  end: Square,
  condition: GitBranch,
  delay: Clock3,
  log: List,
};

/** 右键菜单中各节点类型的图标色。 */
const NODE_ICON_TONES: Readonly<Record<string, string>> = {
  start: 'bg-emerald-100 text-emerald-700',
  end: 'bg-rose-100 text-rose-700',
  condition: 'bg-violet-100 text-violet-700',
  delay: 'bg-orange-100 text-orange-700',
  log: 'bg-blue-100 text-blue-700',
};

const HORIZONTAL_ALIGN_ACTIONS = [
  { kind: 'align', label: '左对齐', icon: AlignStartVertical, mode: 'left' },
  { kind: 'align', label: '居中', icon: AlignCenterVertical, mode: 'center-x' },
  { kind: 'align', label: '右对齐', icon: AlignEndVertical, mode: 'right' },
] as const satisfies ReadonlyArray<ArrangeAction>;

const VERTICAL_ALIGN_ACTIONS = [
  { kind: 'align', label: '顶部', icon: AlignStartHorizontal, mode: 'top' },
  { kind: 'align', label: '居中', icon: AlignCenterHorizontal, mode: 'center-y' },
  { kind: 'align', label: '底部', icon: AlignEndHorizontal, mode: 'bottom' },
] as const satisfies ReadonlyArray<ArrangeAction>;

const DISTRIBUTE_ACTIONS = [
  {
    kind: 'distribute',
    label: '水平分布',
    icon: AlignVerticalSpaceBetween,
    mode: 'horizontal',
  },
  {
    kind: 'distribute',
    label: '垂直分布',
    icon: AlignHorizontalSpaceBetween,
    mode: 'vertical',
  },
] as const satisfies ReadonlyArray<ArrangeAction>;

/** 一级菜单项的统一 Tailwind 样式。 */
const MENU_ITEM_CLASS_NAME = [
  'relative flex min-h-10 w-full items-center gap-2 rounded-lg border-0',
  'bg-transparent px-2 py-1 text-left text-[13px] text-slate-700',
  'hover:bg-blue-50 hover:text-blue-700',
  'disabled:cursor-not-allowed disabled:opacity-50',
].join(' ');

/** 主菜单浮层样式。 */
const CONTEXT_MENU_CLASS_NAME = [
  'absolute z-[120] w-[220px] rounded-[14px] border border-slate-300/90',
  'bg-white/98 p-2 text-slate-800 backdrop-blur-xl',
  'shadow-[0_20px_48px_rgba(28,43,65,.18),0_3px_10px_rgba(28,43,65,.08)]',
].join(' ');

/** 二级排列菜单浮层样式。 */
const ARRANGE_MENU_CLASS_NAME = [
  'absolute -bottom-2 w-[238px] rounded-[14px] border border-slate-300/90',
  'bg-white p-2',
  'shadow-[0_20px_48px_rgba(28,43,65,.16),0_3px_10px_rgba(28,43,65,.07)]',
].join(' ');

/** 二级排列按钮样式。 */
const ARRANGE_ACTION_CLASS_NAME = [
  'flex h-14 min-w-0 flex-col items-center justify-center gap-1 rounded-lg',
  'bg-slate-50 px-1 py-1 text-slate-600 hover:bg-blue-50 hover:text-blue-700',
].join(' ');

/** 菜单分组标题样式。 */
const MENU_HEADING_CLASS_NAME = [
  'flex min-h-[26px] items-center gap-2 px-2 py-0.5 text-[11px]',
  'font-extrabold tracking-[.06em] text-slate-500',
].join(' ');

/** 排列入口图标容器样式。 */
const ARRANGE_TRIGGER_ICON_CLASS_NAME = [
  'flex size-7 shrink-0 items-center justify-center rounded-lg',
  'bg-blue-100 text-blue-700',
].join(' ');

/** 画布右键菜单：提供节点创建，并在多选时提供排列与对齐二级菜单。 */
export function FlowContextMenu({
  context,
  registry,
  nodes,
  onAddNode,
  onClose,
}: FlowContextMenuProps) {
  const [arrangeOpen, setArrangeOpen] = useState(false);
  const selectedCount = useFlowStore((state) => state.selectedNodeIds.size);
  const align = useFlowStore((state) => state.align);
  const distribute = useFlowStore((state) => state.distribute);
  const openArrangeMenu = () => setArrangeOpen(true);
  const closeArrangeMenu = () => setArrangeOpen(false);
  const addNode = (definition: NodeDefinition) => {
    onAddNode(definition.kind, context.world);
    onClose();
  };
  const runArrangeAction = (action: ArrangeAction) => {
    if (action.kind === 'align') align(action.mode);
    else distribute(action.mode);
    onClose();
  };

  return (
    <div
      role="menu"
      className={CONTEXT_MENU_CLASS_NAME}
      style={{ left: context.x, top: context.y }}
      onPointerDown={(event) => event.stopPropagation()}
    >
      <MenuHeading
        icon={Plus}
        label="添加节点"
      />
      <div className="flex flex-col gap-0.5">
        {Object.values(registry).map((definition) => (
          <NodeMenuItem
            key={definition.kind}
            definition={definition}
            disabled={Boolean(
              definition.singleton
              && nodes.some((node) => node.kind === definition.kind),
            )}
            onAdd={addNode}
          />
        ))}
      </div>
      {selectedCount > 1 ? (
        <>
          <div className="mx-1 my-1.5 h-px bg-slate-200" />
          <div
            className="relative"
            onMouseEnter={openArrangeMenu}
            onMouseLeave={closeArrangeMenu}
          >
            <button
              type="button"
              aria-expanded={arrangeOpen}
              aria-haspopup="menu"
              className={`${MENU_ITEM_CLASS_NAME} ${arrangeOpen ? 'bg-blue-50 text-blue-700' : ''}`}
              role="menuitem"
              onClick={openArrangeMenu}
            >
              <span className={ARRANGE_TRIGGER_ICON_CLASS_NAME}>
                <LayoutPanelLeft
                  aria-hidden="true"
                  className="size-5"
                />
              </span>
              <span className="min-w-0 flex-1">排列与对齐</span>
              <small className="ml-auto text-[10px] text-slate-400">
                {selectedCount} 个
              </small>
              <ChevronRight
                aria-hidden="true"
                className="size-5 shrink-0 text-slate-400"
              />
            </button>
            <ArrangeSubmenu
              action={runArrangeAction}
              open={arrangeOpen}
              side={context.submenuSide}
            />
          </div>
        </>
      ) : null}
    </div>
  );
}

type MenuHeadingProps = Readonly<{
  icon?: LucideIcon;
  label: string;
}>;

/** 渲染右键菜单的分组标题。 */
function MenuHeading({ icon: Icon, label }: MenuHeadingProps) {
  return (
    <div
      className={MENU_HEADING_CLASS_NAME}
    >
      {Icon ? (
        <Icon
          aria-hidden="true"
          className="size-5"
        />
      ) : null}
      <span>{label}</span>
    </div>
  );
}

type NodeMenuItemProps = Readonly<{
  definition: NodeDefinition;
  disabled: boolean;
  onAdd: (definition: NodeDefinition) => void;
}>;

/** 渲染一项可创建的业务节点。 */
function NodeMenuItem({ definition, disabled, onAdd }: NodeMenuItemProps) {
  const Icon = NODE_ICONS[definition.kind] ?? Plus;
  const iconTone = NODE_ICON_TONES[definition.kind] ?? 'bg-blue-100 text-blue-700';
  const handleClick = () => onAdd(definition);

  return (
    <button
      type="button"
      className={MENU_ITEM_CLASS_NAME}
      disabled={disabled}
      role="menuitem"
      onClick={handleClick}
    >
      <span className={`flex size-7 shrink-0 items-center justify-center rounded-lg ${iconTone}`}>
        <Icon
          aria-hidden="true"
          className="size-5"
        />
      </span>
      <span className="min-w-0 flex-1">{definition.title}</span>
      {disabled ? (
        <small className="ml-auto text-[10px] text-slate-400">已添加</small>
      ) : null}
    </button>
  );
}

type ArrangeSubmenuProps = Readonly<{
  action: (action: ArrangeAction) => void;
  open: boolean;
  side: CanvasContextMenu['submenuSide'];
}>;

/** 渲染右键菜单的二级排列操作面板。 */
function ArrangeSubmenu({ action, open, side }: ArrangeSubmenuProps) {
  const visibilityClassName = open ? 'block' : 'hidden';
  const sideClassName = side === 'left' ? 'right-full' : 'left-full';

  return (
    <div
      role="menu"
      className={`${visibilityClassName} ${sideClassName} ${ARRANGE_MENU_CLASS_NAME}`}
    >
      <MenuHeading label="水平对齐" />
      <ArrangeActionGroup
        actions={HORIZONTAL_ALIGN_ACTIONS}
        className="mb-1 grid grid-cols-3 gap-1"
        onAction={action}
      />
      <MenuHeading label="垂直对齐" />
      <ArrangeActionGroup
        actions={VERTICAL_ALIGN_ACTIONS}
        className="mb-1 grid grid-cols-3 gap-1"
        onAction={action}
      />
      <MenuHeading label="均匀分布" />
      <ArrangeActionGroup
        actions={DISTRIBUTE_ACTIONS}
        className="grid grid-cols-2 gap-1"
        onAction={action}
      />
    </div>
  );
}

type ArrangeActionGroupProps = Readonly<{
  actions: ReadonlyArray<ArrangeAction>;
  className: string;
  onAction: (action: ArrangeAction) => void;
}>;

/** 按配置渲染同一排列操作分组。 */
function ArrangeActionGroup({
  actions,
  className,
  onAction,
}: ArrangeActionGroupProps) {
  return (
    <div className={className}>
      {actions.map((action) => (
        <MenuAction
          key={`${action.kind}-${action.mode}`}
          action={action}
          onAction={onAction}
        />
      ))}
    </div>
  );
}

type MenuActionProps = Readonly<{
  action: ArrangeAction;
  onAction: (action: ArrangeAction) => void;
}>;

/** 二级菜单中的图标化排列操作。 */
function MenuAction({ action, onAction }: MenuActionProps) {
  const Icon = action.icon;
  const handleClick = () => onAction(action);

  return (
    <button
      type="button"
      className={ARRANGE_ACTION_CLASS_NAME}
      role="menuitem"
      title={action.label}
      onClick={handleClick}
    >
      <Icon
        aria-hidden="true"
        className="size-5"
      />
      <span className="max-w-full truncate text-[11px] leading-none">
        {action.label}
      </span>
    </button>
  );
}
