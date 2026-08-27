import {
  Redo2,
  Undo2,
} from 'lucide-react';
import { useStore, type StoreApi } from 'zustand';

import type { FlowState } from '../../../../flow';
import type {
  WorkflowEdgeData,
  WorkflowNodeData,
} from '../../../../features/workflow';
import { IconButton } from '../../../ui';

type WorkflowFlowStore = StoreApi<FlowState<WorkflowNodeData, WorkflowEdgeData>>;

type EditorToolbarControlsProps = Readonly<{
  /** 当前工作流画布 Store。 */
  store: WorkflowFlowStore;
}>;

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
      <IconButton
        label="撤销"
        icon={Undo2}
        iconClassName="size-4"
        title="撤销 (Ctrl+Z)"
        className="size-7"
        disabled={pastCount === 0}
        onClick={() => store.getState().undo()}
      />
      <IconButton
        label="重做"
        icon={Redo2}
        iconClassName="size-4"
        title="重做 (Ctrl+Y)"
        className="size-7"
        disabled={futureCount === 0}
        onClick={() => store.getState().redo()}
      />
    </nav>
  );
}
