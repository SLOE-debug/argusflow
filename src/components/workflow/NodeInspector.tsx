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
    <aside className="argus-sidebar border-l p-5">
      <p className="argus-section-label font-bold tracking-[0.16em] uppercase">属性</p>
      {!node && !selectedEdgeId && (
        <EmptyInspector message="选择一个节点或连线以查看属性" />
      )}
      {selectedEdgeId && !node && (
        <div className="mt-5">
          <p className="argus-body text-sm font-semibold">已选择连线</p>
          <p className="argus-muted mt-1.5 break-all text-xs">{selectedEdgeId}</p>
          <DeleteButton onClick={onDelete} />
        </div>
      )}
      {node && (
        <div className="mt-5 space-y-5">
          <div>
            <label className="argus-muted text-xs font-medium">节点类型</label>
            <div className="argus-readonly-field mt-1.5 px-3 py-2.5 text-sm">
              {node.data.label}
            </div>
          </div>
          <div>
            <label className="argus-muted text-xs font-medium">节点 ID</label>
            <div className="argus-muted mt-1.5 break-all text-xs">{node.id}</div>
          </div>
          {node.data.kind === 'log' && (
            <div>
              <label htmlFor="log-message" className="argus-muted text-xs font-medium">
                日志内容
              </label>
              <textarea
                id="log-message"
                value={node.data.message ?? ''}
                onChange={(event) => onUpdate({ message: event.target.value, invalid: false })}
                rows={4}
                className="argus-input mt-1.5 w-full resize-none px-3 py-2.5 text-sm placeholder:text-[var(--color-text-subtle)]"
              />
            </div>
          )}
          {node.data.kind === 'delay' && (
            <div>
              <label htmlFor="delay-duration" className="argus-muted text-xs font-medium">
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
                className="argus-input mt-1.5 w-full px-3 py-2.5 text-sm"
              />
            </div>
          )}
          {canDelete && <DeleteButton onClick={onDelete} />}
          {!canDelete && (
            <p className="argus-callout rounded-lg px-3 py-2.5 text-xs">
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
    <div className="argus-empty-state mt-5 rounded-xl border border-dashed px-4 py-8 text-center text-xs leading-5">
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
      className="argus-button-danger mt-4 w-full px-3 py-2.5 text-sm font-semibold"
    >
      删除选中项
    </button>
  );
}
