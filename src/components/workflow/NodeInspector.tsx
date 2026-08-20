import type { WorkflowCanvasNode } from '../../features/workflow/workflowModel';

type NodeInspectorProps = {
  /** 当前选中的节点；无节点时可通过 selectedEdgeId 展示边属性。 */
  node: WorkflowCanvasNode | null;
  /** 当前选中的边 ID，与节点选择互斥。 */
  selectedEdgeId: string | null;
  /** 当前选择是否允许删除。Start/End 固定节点不可删除。 */
  canDelete: boolean;
  /** 更新当前节点的可编辑数据字段。 */
  onUpdate: (data: Partial<WorkflowCanvasNode['data']>) => void;
  /** 删除当前选中节点或边。 */
  onDelete: () => void;
};

/** 展示选中节点/连线属性，并提供受约束的编辑和删除操作。 */
export function NodeInspector({
  node,
  selectedEdgeId,
  canDelete,
  onUpdate,
  onDelete,
}: NodeInspectorProps) {
  return (
    <aside className="border-l border-[#1d3048] bg-[#0c1828] p-4">
      <p className="text-[11px] font-semibold tracking-[0.2em] text-sky-400 uppercase">属性</p>
      {!node && !selectedEdgeId && (
        <EmptyInspector message="选择一个节点或连线以查看属性" />
      )}
      {selectedEdgeId && !node && (
        <div className="mt-4">
          <p className="text-sm font-medium text-slate-200">已选择连线</p>
          <p className="mt-1 break-all text-xs text-slate-500">{selectedEdgeId}</p>
          <DeleteButton onClick={onDelete} />
        </div>
      )}
      {node && (
        <div className="mt-4 space-y-4">
          <div>
            <label className="text-xs text-slate-500">节点类型</label>
            <div className="mt-1 rounded-lg border border-[#263d57] bg-[#101f33] px-3 py-2 text-sm text-slate-200">
              {node.data.label}
            </div>
          </div>
          <div>
            <label className="text-xs text-slate-500">节点 ID</label>
            <div className="mt-1 break-all text-xs text-slate-400">{node.id}</div>
          </div>
          {node.data.kind === 'log' && (
            <div>
              <label htmlFor="log-message" className="text-xs text-slate-500">
                日志内容
              </label>
              <textarea
                id="log-message"
                value={node.data.message ?? ''}
                onChange={(event) => onUpdate({ message: event.target.value, invalid: false })}
                rows={4}
                className="mt-1 w-full resize-none rounded-lg border border-[#263d57] bg-[#101f33] px-3 py-2 text-sm text-slate-200 placeholder:text-slate-600"
              />
            </div>
          )}
          {node.data.kind === 'delay' && (
            <div>
              <label htmlFor="delay-duration" className="text-xs text-slate-500">
                等待毫秒（1–60000）
              </label>
              <input
                id="delay-duration"
                type="number"
                min={1}
                max={60_000}
                value={node.data.milliseconds ?? 0}
                onChange={(event) =>
                  onUpdate({ milliseconds: Number(event.target.value), invalid: false })
                }
                className="mt-1 w-full rounded-lg border border-[#263d57] bg-[#101f33] px-3 py-2 text-sm text-slate-200"
              />
            </div>
          )}
          {canDelete && <DeleteButton onClick={onDelete} />}
          {!canDelete && (
            <p className="rounded-lg bg-[#101f33] px-3 py-2 text-xs text-slate-500">
              Start 和 End 是首版工作流的固定节点。
            </p>
          )}
        </div>
      )}
    </aside>
  );
}

/** 未选中画布元素时的属性面板占位内容。 */
function EmptyInspector({ message }: { message: string }) {
  return (
    <div className="mt-4 rounded-xl border border-dashed border-[#29415e] px-3 py-6 text-center text-xs leading-5 text-slate-500">
      {message}
    </div>
  );
}

/** 触发删除当前选择项的统一按钮。 */
function DeleteButton({ onClick }: { onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="mt-4 w-full rounded-lg border border-rose-400/30 bg-rose-400/8 px-3 py-2 text-xs font-medium text-rose-300 transition hover:bg-rose-400/15"
    >
      删除选中项
    </button>
  );
}
