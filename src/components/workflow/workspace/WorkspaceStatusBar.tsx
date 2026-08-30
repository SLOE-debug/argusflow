import type { CSSProperties } from 'react';
import { useStore, type StoreApi } from 'zustand';

import type { FlowState } from '../../../flow';
import type { WorkflowEdgeData, WorkflowNodeData } from '../../../features/workflow';
import { runtimeStatusLabel, type StartupSnapshot } from '../../../features/startup';
import type { WorkflowStatusPresentation } from '../overview/workflowStatus';

type WorkspaceStatusBarProps = Readonly<{
  /** 当前工作流画布 Store。 */
  store: StoreApi<FlowState<WorkflowNodeData, WorkflowEdgeData>>;
  /** 当前运行与校验状态。 */
  status: WorkflowStatusPresentation;
  /** 桌面捕获与 OCR 的实时运行环境状态。 */
  runtimeStatus: StartupSnapshot;
  /** 左侧面板当前宽度；关闭时为 null。 */
  libraryWidth: number | null;
  /** 右侧面板当前宽度；关闭时为 null。 */
  inspectorWidth: number | null;
}>;

/** 展示真实画布统计与运行状态的底部状态栏。 */
export function WorkspaceStatusBar({
  store,
  status,
  runtimeStatus,
  libraryWidth,
  inspectorWidth,
}: WorkspaceStatusBarProps) {
  const nodeCount = useStore(store, (state) => state.nodes.length);
  /** 底部状态栏与可调宽工作区保持列边界对齐。 */
  const gridStyle: CSSProperties = {
    gridTemplateColumns: [
      libraryWidth === null ? null : `${libraryWidth}px`,
      'minmax(0, 1fr)',
      inspectorWidth === null ? null : `${inspectorWidth}px`,
    ].filter((column): column is string => column !== null).join(' '),
  };

  return (
    <footer
      className="z-20 grid h-10 items-center border-t border-slate-200 bg-slate-50 text-[11px] text-slate-500"
      style={gridStyle}
    >
      {libraryWidth !== null ? (
        <span className="px-4">{nodeCount} 个节点</span>
      ) : null}
      <div className="flex items-center justify-end px-4">
        <RuntimeSummary status={runtimeStatus} />
        {inspectorWidth === null ? (
          <StatusSummary status={status} />
        ) : null}
      </div>
      {inspectorWidth !== null ? (
        <div className="flex items-center justify-end px-4">
          <StatusSummary status={status} />
        </div>
      ) : null}
    </footer>
  );
}

/** 显示实际 CPU/GPU 设备以及关键能力是否可运行。 */
function RuntimeSummary({ status }: Readonly<{ status: StartupSnapshot }>) {
  const tone = status.readiness === 'ready'
    ? status.degradationReason ? 'bg-amber-500' : 'bg-emerald-500'
    : status.readiness === 'blocked'
      ? 'bg-rose-500'
      : 'bg-blue-500';
  return (
    <span className="flex items-center">
      <span className={`mr-2 size-2 rounded-full ${tone}`} />
      <span>{runtimeStatusLabel(status)}</span>
    </span>
  );
}

/** 可在中央或右侧列复用的工作流状态摘要。 */
function StatusSummary({ status }: Readonly<{ status: WorkflowStatusPresentation }>) {
  return (
    <span className="ml-4 flex items-center">
      <span className={`mr-2 size-2 rounded-full ${status.tone}`} />
      <span>{status.label}</span>
    </span>
  );
}
