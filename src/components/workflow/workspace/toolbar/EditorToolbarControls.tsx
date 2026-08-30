import PanelBottom from 'lucide-react/dist/esm/icons/panel-bottom.mjs';
import PanelLeft from 'lucide-react/dist/esm/icons/panel-left.mjs';
import PanelRight from 'lucide-react/dist/esm/icons/panel-right.mjs';
import Redo2 from 'lucide-react/dist/esm/icons/redo-2.mjs';
import Undo2 from 'lucide-react/dist/esm/icons/undo-2.mjs';
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
  /** 左侧节点/预设面板是否展开。 */
  libraryOpen: boolean;
  /** 底部执行与结构化编辑 Dock 是否展开。 */
  dockOpen: boolean;
  /** 右侧属性面板是否展开。 */
  inspectorOpen: boolean;
  /** 切换左侧面板。 */
  onLibraryOpenChange: (open: boolean) => void;
  /** 切换底部 Dock。 */
  onDockOpenChange: (open: boolean) => void;
  /** 切换右侧面板。 */
  onInspectorOpenChange: (open: boolean) => void;
}>;

/** 可直接插入标题栏的编辑和面板命令，不承担任何整行布局。 */
export function EditorToolbarControls({
  store,
  libraryOpen,
  dockOpen,
  inspectorOpen,
  onLibraryOpenChange,
  onDockOpenChange,
  onInspectorOpenChange,
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
      <span className="mx-1 h-4 w-px bg-slate-200" aria-hidden="true" />
      <IconButton
        label="左侧面板"
        icon={PanelLeft}
        iconClassName="size-3.5"
        className="size-7"
        aria-pressed={libraryOpen}
        title={libraryOpen ? '收起左侧面板' : '展开左侧面板'}
        onClick={() => onLibraryOpenChange(!libraryOpen)}
      />
      <IconButton
        label="底部面板"
        icon={PanelBottom}
        iconClassName="size-3.5"
        className="size-7"
        aria-pressed={dockOpen}
        title={dockOpen ? '收起底部面板' : '展开底部面板'}
        onClick={() => onDockOpenChange(!dockOpen)}
      />
      <IconButton
        label="右侧面板"
        icon={PanelRight}
        iconClassName="size-3.5"
        className="size-7"
        aria-pressed={inspectorOpen}
        title={inspectorOpen ? '收起右侧面板' : '展开右侧面板'}
        onClick={() => onInspectorOpenChange(!inspectorOpen)}
      />
    </nav>
  );
}
