import {
  Redo2,
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
  /** 点击回调。 */
  onClick: () => void;
}>;

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
}: EditorToolbarControlsProps) {
  const pastCount = useStore(store, (state) => state.past.length);
  const futureCount = useStore(store, (state) => state.future.length);

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
    </nav>
  );
}

/** 标题栏中的统一图标操作。 */
function CommandIconButton({
  label,
  shortcut,
  icon: Icon,
  disabled = false,
  onClick,
}: CommandIconButtonProps) {
  const title = shortcut ? `${label} (${shortcut})` : label;

  return (
    <button
      type="button"
      className={COMMAND_BUTTON_CLASS_NAME}
      aria-label={label}
      title={title}
      disabled={disabled}
      onClick={onClick}
    >
      <Icon className="size-3.5" aria-hidden="true" />
    </button>
  );
}
