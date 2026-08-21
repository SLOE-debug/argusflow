import {
  ChevronDown,
  ClipboardPaste,
  Copy,
  CopyPlus,
  PanelBottom,
  PanelLeft,
  PanelRight,
  Play,
  Redo2,
  ShieldCheck,
  Trash2,
  Undo2,
  Upload,
  type LucideIcon,
} from 'lucide-react';
import { useStore, type StoreApi } from 'zustand';

import type { FlowState } from '../../flow';
import type {
  WorkflowEdgeData,
  WorkflowNodeData,
} from '../../features/workflow/workflowModel';

type WorkflowFlowStore = StoreApi<FlowState<WorkflowNodeData, WorkflowEdgeData>>;

type EditorCommandBarProps = Readonly<{
  /** 当前工作流画布 Store。 */
  store: WorkflowFlowStore;
  /** 后端运行是否正在进行。 */
  running: boolean;
  /** 左侧节点库是否可见。 */
  libraryOpen: boolean;
  /** 右侧检查器是否可见。 */
  inspectorOpen: boolean;
  /** 底部运行面板是否可见。 */
  consoleOpen: boolean;
  /** 请求结构校验。 */
  onValidate: () => void;
  /** 请求运行当前工作流。 */
  onRun: () => void;
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

/** 统一的 32px 桌面命令按钮样式。 */
const COMMAND_BUTTON_CLASS_NAME = [
  'flex size-8 items-center justify-center rounded-[4px] border-0',
  'bg-transparent text-slate-600 outline-none hover:bg-slate-100',
  'hover:text-slate-900 focus-visible:ring-2 focus-visible:ring-blue-500',
  'disabled:cursor-default disabled:opacity-35 disabled:hover:bg-transparent',
].join(' ');

/** 接入 Flow Store 现有编辑能力的高密度命令栏。 */
export function EditorCommandBar({
  store,
  running,
  libraryOpen,
  inspectorOpen,
  consoleOpen,
  onValidate,
  onRun,
  onToggleLibrary,
  onToggleInspector,
  onToggleConsole,
}: EditorCommandBarProps) {
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
      className="z-20 flex h-[52px] items-center gap-1 border-b border-slate-200 bg-white pr-4 pl-[196px]"
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
      <div className="ml-auto flex items-center gap-2.5">
        <button
          type="button"
          className="flex h-9 items-center gap-2 rounded-md border border-slate-300 bg-white px-5 text-[13px] font-medium text-slate-700 outline-none hover:bg-slate-50 focus-visible:ring-2 focus-visible:ring-blue-500 disabled:opacity-40"
          onClick={onValidate}
          disabled={running}
          aria-label="校验"
        >
          <ShieldCheck className="size-4" aria-hidden="true" />
          校验
        </button>
        <SplitActionButton
          label={running ? '运行中…' : '运行'}
          icon={Play}
          disabled={running}
          onClick={onRun}
        />
        <SplitActionButton
          label="发布"
          icon={Upload}
          onClick={() => undefined}
        />
      </div>
    </nav>
  );
}

/** 桌面命令栏中的统一图标操作。 */
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
      <Icon className="size-4" aria-hidden="true" />
    </button>
  );
}

/** 命令分组之间的细分隔线。 */
function CommandSeparator() {
  return <span className="mx-2 h-6 w-px bg-slate-200" />;
}

type SplitActionButtonProps = Readonly<{
  /** 主按钮文字，同时作为可访问名称。 */
  label: string;
  /** 主按钮 Lucide 图标。 */
  icon: LucideIcon;
  /** 当前是否不可用。 */
  disabled?: boolean;
  /** 主操作回调。 */
  onClick: () => void;
}>;

/** 参考图右侧带下拉分区的蓝色主操作。 */
function SplitActionButton({
  label,
  icon: Icon,
  disabled = false,
  onClick,
}: SplitActionButtonProps) {
  return (
    <div className="flex h-9 overflow-hidden rounded-md bg-blue-600 text-white shadow-sm">
      <button
        type="button"
        className="flex h-9 items-center gap-2 px-4 text-[13px] font-semibold outline-none hover:bg-blue-700 disabled:cursor-default disabled:opacity-45"
        onClick={onClick}
        disabled={disabled}
        aria-label={label}
      >
        <Icon className="size-4" aria-hidden="true" />
        {label}
      </button>
      <button
        type="button"
        aria-label={`${label}选项`}
        disabled={disabled}
        className="flex h-9 w-9 items-center justify-center border-l border-blue-500 hover:bg-blue-700 disabled:opacity-45"
      >
        <ChevronDown className="size-3.5" aria-hidden="true" />
      </button>
    </div>
  );
}
