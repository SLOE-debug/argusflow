import { useStore, type StoreApi } from 'zustand';

import type { FlowState } from '../../flow';
import type { WorkflowEdgeData, WorkflowNodeData } from '../../features/workflow/workflowModel';
import type { WorkflowStatusPresentation } from './workflowStatus';

type WorkspaceStatusBarProps = Readonly<{
  /** 当前工作流画布 Store。 */
  store: StoreApi<FlowState<WorkflowNodeData, WorkflowEdgeData>>;
  /** 当前运行与校验状态。 */
  status: WorkflowStatusPresentation;
}>;

/** 展示真实画布统计与运行状态的底部状态栏。 */
export function WorkspaceStatusBar({ store, status }: WorkspaceStatusBarProps) {
  const nodeCount = useStore(store, (state) => state.nodes.length);

  return (
    <footer className="z-20 grid h-10 grid-cols-[224px_minmax(0,1fr)_336px] items-center border-t border-slate-200 bg-slate-50 text-[11px] text-slate-500">
      <span className="px-4">{nodeCount} 项</span>
      <div className="flex items-center justify-end px-4">
        <span>Rust 引擎 1.3.0</span>
        <span className="mx-3 h-3 w-px bg-slate-300" />
        <span>高性能模式</span>
        <span className="mx-3 h-3 w-px bg-slate-300" />
        <span>资源占用：CPU 2%</span>
        <span className="ml-4">内存 128MB</span>
      </div>
      <div className="flex items-center justify-end px-4">
        <span className={`mr-2 size-2 rounded-full ${status.tone}`} />
        <span>{status.label}</span>
      </div>
    </footer>
  );
}
