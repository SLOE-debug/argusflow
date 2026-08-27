import { useEffect, useState } from 'react';
import {
  AlignCenterHorizontal,
  AlignCenterVertical,
  AlignEndHorizontal,
  AlignEndVertical,
  AlignHorizontalSpaceBetween,
  AlignStartHorizontal,
  AlignStartVertical,
  AlignVerticalSpaceBetween,
  CirclePlay,
  ClipboardPaste,
  Clock3,
  Copy,
  CopyPlus,
  GitBranch,
  LayoutPanelLeft,
  List,
  Plus,
  Redo2,
  Square,
  Trash2,
  Undo2,
  type LucideIcon,
} from 'lucide-react';

import { FlowMenuItem, FlowMenuSeparator, FlowMenuSurface } from './FlowMenu';
import type { AlignMode, DistributeMode } from './selection';
import { useFlowStore } from './store';
import type {
  FlowAnchorSide,
  FlowNode,
  NodeDefinition,
  NodeRegistry,
} from './types';
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
  /** 从连线落点新建节点，并一次完成连线。 */
  onAddConnectedNode: (
    kind: string,
    position: CanvasContextMenu['world'],
    sourceNodeId: string,
    sourceSide: FlowAnchorSide,
  ) => boolean;
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

type OpenSubmenu = 'nodes' | 'arrange' | null;

/** 各级菜单的稳定键盘焦点标识。 */
const MENU_IDS = {
  root: 'flow-context-root',
  nodes: 'flow-context-nodes',
  arrange: 'flow-context-arrange',
  connection: 'flow-context-connection',
} as const;

/** 右键菜单内业务节点类型对应的图标。 */
const NODE_ICONS: Readonly<Record<string, LucideIcon>> = {
  start: CirclePlay,
  end: Square,
  condition: GitBranch,
  delay: Clock3,
  log: List,
};

/** 节点图标使用的单一强调色，不再绘制彩色图标底板。 */
const NODE_ICON_TONES: Readonly<Record<string, string>> = {
  start: 'text-emerald-700',
  end: 'text-rose-700',
  condition: 'text-violet-700',
  delay: 'text-orange-700',
  log: 'text-blue-700',
};

const HORIZONTAL_ALIGN_ACTIONS = [
  { kind: 'align', label: '左对齐', icon: AlignStartVertical, mode: 'left' },
  { kind: 'align', label: '水平居中', icon: AlignCenterVertical, mode: 'center-x' },
  { kind: 'align', label: '右对齐', icon: AlignEndVertical, mode: 'right' },
] as const satisfies ReadonlyArray<ArrangeAction>;

const VERTICAL_ALIGN_ACTIONS = [
  { kind: 'align', label: '顶部对齐', icon: AlignStartHorizontal, mode: 'top' },
  { kind: 'align', label: '垂直居中', icon: AlignCenterHorizontal, mode: 'center-y' },
  { kind: 'align', label: '底部对齐', icon: AlignEndHorizontal, mode: 'bottom' },
] as const satisfies ReadonlyArray<ArrangeAction>;

const DISTRIBUTE_ACTIONS = [
  { kind: 'distribute', label: '水平分布', icon: AlignVerticalSpaceBetween, mode: 'horizontal' },
  { kind: 'distribute', label: '垂直分布', icon: AlignHorizontalSpaceBetween, mode: 'vertical' },
] as const satisfies ReadonlyArray<ArrangeAction>;

/** Windows 风格画布右键菜单，提供现有编辑、创建和排列能力。 */
export function FlowContextMenu({
  context,
  registry,
  nodes,
  onAddNode,
  onAddConnectedNode,
  onClose,
}: FlowContextMenuProps) {
  const [openSubmenu, setOpenSubmenu] = useState<OpenSubmenu>(null);
  const [connectionError, setConnectionError] = useState<string | null>(null);
  const selectedNodeCount = useFlowStore((state) => state.selectedNodeIds.size);
  const selectedEdgeId = useFlowStore((state) => state.selectedEdgeId);
  const pastCount = useFlowStore((state) => state.past.length);
  const futureCount = useFlowStore((state) => state.future.length);
  const hasClipboard = useFlowStore((state) => state.clipboard !== null);
  const undo = useFlowStore((state) => state.undo);
  const redo = useFlowStore((state) => state.redo);
  const copy = useFlowStore((state) => state.copy);
  const paste = useFlowStore((state) => state.paste);
  const duplicate = useFlowStore((state) => state.duplicate);
  const deleteSelection = useFlowStore((state) => state.deleteSelection);
  const align = useFlowStore((state) => state.align);
  const distribute = useFlowStore((state) => state.distribute);
  const hasSelection = selectedNodeCount > 0 || selectedEdgeId !== null;
  /** 注册表中声明为单例的节点类型。 */
  const singletonKinds = new Set(
    Object.values(registry)
      .filter((definition) => definition.singleton)
      .map((definition) => definition.kind),
  );
  const submenuSideClassName = context.submenuSide === 'left'
    ? 'right-full mr-1'
    : 'left-full ml-1';

  useEffect(() => {
    setOpenSubmenu(null);
    setConnectionError(null);
    const menuId = context.pendingConnection
      ? MENU_IDS.connection
      : MENU_IDS.root;
    queueMicrotask(() => {
      document.querySelector<HTMLButtonElement>(
        `[data-menu-owner="${menuId}"]:not(:disabled)`,
      )?.focus();
    });
  }, [context]);

  /** 执行无参数 Store 命令并关闭菜单。 */
  const runCommand = (command: () => void) => {
    command();
    onClose();
  };
  const addNode = (definition: NodeDefinition) => {
    onAddNode(definition.kind, context.world);
    onClose();
  };
  const addConnectedNode = (definition: NodeDefinition) => {
    const pendingConnection = context.pendingConnection;
    if (!pendingConnection) return;

    /** 菜单落点对准新节点中心，使拖线位置与布局结果一致。 */
    const position = {
      x: Math.round(context.world.x - definition.defaultSize.width / 2),
      y: Math.round(context.world.y - definition.defaultSize.height / 2),
    };
    const added = onAddConnectedNode(
      definition.kind,
      position,
      pendingConnection.sourceNodeId,
      pendingConnection.sourceSide,
    );
    if (added) onClose();
    else setConnectionError('当前节点已达到可连接的数量上限。');
  };
  const runArrangeAction = (action: ArrangeAction) => {
    if (action.kind === 'align') align(action.mode);
    else distribute(action.mode);
    onClose();
  };
  const closeSubmenu = () => {
    const expandedTrigger = document.querySelector<HTMLButtonElement>(
      `[data-menu-owner="${MENU_IDS.root}"][aria-expanded="true"]`,
    );
    setOpenSubmenu(null);
    queueMicrotask(() => expandedTrigger?.focus());
  };

  if (context.pendingConnection) {
    return (
      <FlowMenuSurface
        menuId={MENU_IDS.connection}
        ariaLabel="添加并连接节点"
        className="absolute"
        style={{ left: context.x, top: context.y }}
        onBack={onClose}
      >
        <NodeMenuItems
          menuId={MENU_IDS.connection}
          registry={registry}
          nodes={nodes}
          requireConnectionTarget
          onAdd={addConnectedNode}
        />
        {connectionError ? (
          <p
            role="status"
            className="mx-1 mt-1 rounded bg-rose-50 px-2 py-1.5 text-[10px] leading-4 text-rose-700"
          >
            {connectionError}
          </p>
        ) : null}
      </FlowMenuSurface>
    );
  }

  return (
    <FlowMenuSurface
      menuId={MENU_IDS.root}
      ariaLabel="画布菜单"
      className="absolute"
      style={{ left: context.x, top: context.y }}
      onBack={onClose}
    >
      <FlowMenuItem
        menuId={MENU_IDS.root}
        label="撤销"
        shortcut="Ctrl+Z"
        icon={Undo2}
        disabled={pastCount === 0}
        onHighlight={() => setOpenSubmenu(null)}
        onClick={() => runCommand(undo)}
      />
      <FlowMenuItem
        menuId={MENU_IDS.root}
        label="重做"
        shortcut="Ctrl+Y"
        icon={Redo2}
        disabled={futureCount === 0}
        onHighlight={() => setOpenSubmenu(null)}
        onClick={() => runCommand(redo)}
      />
      <FlowMenuSeparator />
      <FlowMenuItem
        menuId={MENU_IDS.root}
        label="复制"
        shortcut="Ctrl+C"
        icon={Copy}
        disabled={selectedNodeCount === 0}
        onHighlight={() => setOpenSubmenu(null)}
        onClick={() => runCommand(copy)}
      />
      <FlowMenuItem
        menuId={MENU_IDS.root}
        label="粘贴"
        shortcut="Ctrl+V"
        icon={ClipboardPaste}
        disabled={!hasClipboard}
        onHighlight={() => setOpenSubmenu(null)}
        onClick={() => runCommand(() => paste(singletonKinds))}
      />
      <FlowMenuItem
        menuId={MENU_IDS.root}
        label="创建副本"
        shortcut="Ctrl+D"
        icon={CopyPlus}
        disabled={selectedNodeCount === 0}
        onHighlight={() => setOpenSubmenu(null)}
        onClick={() => runCommand(() => duplicate(singletonKinds))}
      />
      <FlowMenuItem
        menuId={MENU_IDS.root}
        label="删除"
        shortcut="Del"
        icon={Trash2}
        disabled={!hasSelection}
        onHighlight={() => setOpenSubmenu(null)}
        onClick={() => runCommand(() => deleteSelection())}
      />
      <FlowMenuSeparator />
      <FlowMenuItem
        menuId={MENU_IDS.root}
        label="添加节点"
        icon={Plus}
        submenuId={MENU_IDS.nodes}
        submenuOpen={openSubmenu === 'nodes'}
        onOpenSubmenu={() => setOpenSubmenu('nodes')}
      >
        <NodeSubmenu
          className={submenuSideClassName}
          registry={registry}
          nodes={nodes}
          onAdd={addNode}
          onBack={closeSubmenu}
        />
      </FlowMenuItem>
      {selectedNodeCount > 1 ? (
        <FlowMenuItem
          menuId={MENU_IDS.root}
          label="排列与对齐"
          icon={LayoutPanelLeft}
          submenuId={MENU_IDS.arrange}
          submenuOpen={openSubmenu === 'arrange'}
          onOpenSubmenu={() => setOpenSubmenu('arrange')}
        >
          <ArrangeSubmenu
            className={submenuSideClassName}
            onAction={runArrangeAction}
            onBack={closeSubmenu}
          />
        </FlowMenuItem>
      ) : null}
    </FlowMenuSurface>
  );
}

type NodeSubmenuProps = Readonly<{
  className: string;
  registry: Readonly<NodeRegistry>;
  nodes: ReadonlyArray<FlowNode>;
  onAdd: (definition: NodeDefinition) => void;
  onBack: () => void;
}>;

/** 可添加节点的纵向级联菜单。 */
function NodeSubmenu({ className, registry, nodes, onAdd, onBack }: NodeSubmenuProps) {
  return (
    <FlowMenuSurface
      menuId={MENU_IDS.nodes}
      ariaLabel="添加节点"
      className={`absolute bottom-0 ${className}`}
      onBack={onBack}
    >
      <NodeMenuItems
        menuId={MENU_IDS.nodes}
        registry={registry}
        nodes={nodes}
        onAdd={onAdd}
      />
    </FlowMenuSurface>
  );
}

type NodeMenuItemsProps = Readonly<{
  /** 菜单层稳定标识。 */
  menuId: string;
  /** 可创建节点注册表。 */
  registry: Readonly<NodeRegistry>;
  /** 当前节点用于检测单例冲突。 */
  nodes: ReadonlyArray<FlowNode>;
  /** 为 true 时过滤不能接收连线的节点。 */
  requireConnectionTarget?: boolean;
  /** 选择可用节点定义。 */
  onAdd: (definition: NodeDefinition) => void;
}>;

/** 普通添加菜单与连线落点菜单共用的节点项。 */
function NodeMenuItems({
  menuId,
  registry,
  nodes,
  requireConnectionTarget = false,
  onAdd,
}: NodeMenuItemsProps) {
  return Object.values(registry).map((definition) => {
    const Icon = NODE_ICONS[definition.kind] ?? Plus;
    const iconTone = NODE_ICON_TONES[definition.kind] ?? 'text-blue-700';
    const singletonConflict = Boolean(
      definition.singleton && nodes.some((node) => node.kind === definition.kind),
    );
    const disabled = singletonConflict || (
      requireConnectionTarget && definition.canEndConnection === false
    );

    return (
      <FlowMenuItem
        key={definition.kind}
        menuId={menuId}
        label={definition.title}
        icon={Icon}
        iconTone={iconTone}
        disabled={disabled}
        onClick={() => onAdd(definition)}
      />
    );
  });
}

type ArrangeSubmenuProps = Readonly<{
  className: string;
  onAction: (action: ArrangeAction) => void;
  onBack: () => void;
}>;

/** 对齐与分布操作的纵向级联菜单。 */
function ArrangeSubmenu({ className, onAction, onBack }: ArrangeSubmenuProps) {
  return (
    <FlowMenuSurface
      menuId={MENU_IDS.arrange}
      ariaLabel="排列与对齐"
      className={`absolute bottom-0 ${className}`}
      onBack={onBack}
    >
      <ArrangeActionItems
        actions={HORIZONTAL_ALIGN_ACTIONS}
        onAction={onAction}
      />
      <FlowMenuSeparator />
      <ArrangeActionItems
        actions={VERTICAL_ALIGN_ACTIONS}
        onAction={onAction}
      />
      <FlowMenuSeparator />
      <ArrangeActionItems
        actions={DISTRIBUTE_ACTIONS}
        onAction={onAction}
      />
    </FlowMenuSurface>
  );
}

type ArrangeActionItemsProps = Readonly<{
  actions: ReadonlyArray<ArrangeAction>;
  onAction: (action: ArrangeAction) => void;
}>;

/** 将同一分组中的排列动作渲染为菜单项。 */
function ArrangeActionItems({ actions, onAction }: ArrangeActionItemsProps) {
  return actions.map((action) => (
    <FlowMenuItem
      key={`${action.kind}-${action.mode}`}
      menuId={MENU_IDS.arrange}
      label={action.label}
      icon={action.icon}
      onClick={() => onAction(action)}
    />
  ));
}
