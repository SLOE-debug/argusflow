import {
  ClipboardPaste,
  Copy,
  CopyPlus,
  PanelBottom,
  PanelLeft,
  PanelRight,
  Redo2,
  Trash2,
  Undo2,
  type LucideIcon,
} from 'lucide-react';
import { useStore, type StoreApi } from 'zustand';

import type { FlowState } from '../../flow';
import type {
  WorkflowEdgeData,
  WorkflowNodeData,
} from '../../features/workflow/workflowModel';

type WorkflowFlowStore = StoreApi<FlowState<WorkflowNodeData, WorkflowEdgeData>>;

type EditorToolbarControlsProps = Readonly<{
  /** 当前工作流画布 Store。 */
  store: WorkflowFlowStore;
  /** 左侧节点库是否可见。 */
  libraryOpen: boolean;
  /** 右侧检查器是否可见。 */
  inspectorOpen: boolean;
  /** 底部运行面板是否可见。 */
  consoleOpen: boolean;
  /** 切换节点库。 */
  onToggleLibrary: () => void;
  /** 切换检查器。 */
  onToggleInspector: () => void;
  /** 切换运行面板。 */
  onToggleConsole: () => void;
}>;

type CommandIconButtonProps = Readonly<{
  /** 操作名称。 */
  label: string;
  /** Windows 快捷键提示。 */
  shortcut?: string;
  /** Lucide 图标。 */
  icon: LucideIcon;
  /** 操作当前是否不可用。 */
  disabled?: boolean;
  /** 面板切换按钮是否处于按下状态。 */
  pressed?: boolean;
  /** 点击回调。 */
  onClick: () => void;
}>;

/** 工作流中不允许被粘贴为重复实例的节点类型。 */
const SINGLETON_NODE_KINDS: ReadonlyArray<WorkflowNodeData['kind']> = [
  'start',
  'end',
];

/** 标题栏中的统一高密度图标按钮样式。 */
const COMMAND_BUTTON_CLASS_NAME = [
  'flex size-7 items-center justify-center rounded-[4px] border-0',
  'bg-transparent text-slate-600 outline-none hover:bg-slate-100',
  'hover:text-slate-900 focus-visible:ring-2 focus-visible:ring-blue-500',
  'disabled:cursor-default disabled:opacity-35 disabled:hover:bg-transparent',
].join(' ');

/** 可直接插入标题栏的编辑和面板命令，不承担任何整行布局。 */
export function EditorToolbarControls({
  store,
  libraryOpen,
  inspectorOpen,
  consoleOpen,
  onToggleLibrary,
  onToggleInspector,
  onToggleConsole,
}: EditorToolbarControlsProps) {
  const pastCount = useStore(store, (state) => state.past.length);
  const futureCount = useStore(store, (state) => state.future.length);
  const selectedNodeCount = useStore(store, (state) => state.selectedNodeIds.size);
  const selectedEdgeId = useStore(store, (state) => state.selectedEdgeId);
  const hasClipboard = useStore(store, (state) => state.clipboard !== null);
  const hasSelection = selectedNodeCount > 0 || selectedEdgeId !== null;
  const singletonKinds = () => new Set(SINGLETON_NODE_KINDS);

  return (
    <nav
      aria-label="编辑命令"
      className="flex min-w-0 items-center gap-0.5"
    >
      <CommandIconButton
        label="撤销"
        shortcut="Ctrl+Z"
        icon={Undo2}
        disabled={pastCount === 0}
        onClick={() => store.getState().undo()}
      />
      <CommandIconButton
        label="重做"
        shortcut="Ctrl+Y"
        icon={Redo2}
        disabled={futureCount === 0}
        onClick={() => store.getState().redo()}
      />
      <span className="hidden items-center min-[1180px]:flex">
        <CommandSeparator />
        <CommandIconButton
          label="复制"
          shortcut="Ctrl+C"
          icon={Copy}
          disabled={selectedNodeCount === 0}
          onClick={() => store.getState().copy()}
        />
        <CommandIconButton
          label="粘贴"
          shortcut="Ctrl+V"
          icon={ClipboardPaste}
          disabled={!hasClipboard}
          onClick={() => store.getState().paste(singletonKinds())}
        />
        <CommandIconButton
          label="创建副本"
          shortcut="Ctrl+D"
          icon={CopyPlus}
          disabled={selectedNodeCount === 0}
          onClick={() => store.getState().duplicate(singletonKinds())}
        />
        <CommandIconButton
          label="删除"
          shortcut="Delete"
          icon={Trash2}
          disabled={!hasSelection}
          onClick={() => store.getState().deleteSelection()}
        />
      </span>
      <span className="hidden items-center min-[1480px]:flex">
        <CommandSeparator />
        <CommandIconButton
          label="切换节点库"
          icon={PanelLeft}
          pressed={libraryOpen}
          onClick={onToggleLibrary}
        />
        <CommandIconButton
          label="切换运行面板"
          icon={PanelBottom}
          pressed={consoleOpen}
          onClick={onToggleConsole}
        />
        <CommandIconButton
          label="切换属性栏"
          icon={PanelRight}
          pressed={inspectorOpen}
          onClick={onToggleInspector}
        />
      </span>
    </nav>
  );
}

/** 标题栏中的统一图标操作。 */
function CommandIconButton({
  label,
  shortcut,
  icon: Icon,
  disabled = false,
  pressed,
  onClick,
}: CommandIconButtonProps) {
  const title = shortcut ? `${label} (${shortcut})` : label;
  const pressedClassName = pressed ? 'bg-blue-50 text-blue-700' : '';

  return (
    <button
      type="button"
      className={`${COMMAND_BUTTON_CLASS_NAME} ${pressedClassName}`}
      aria-label={label}
      aria-pressed={pressed}
      title={title}
      disabled={disabled}
      onClick={onClick}
    >
      <Icon className="size-3.5" aria-hidden="true" />
    </button>
  );
}

/** 命令分组之间的细分隔线。 */
function CommandSeparator() {
  return <span className="mx-1.5 h-5 w-px bg-slate-200" />;
}
