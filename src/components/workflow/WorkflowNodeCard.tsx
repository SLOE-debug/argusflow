import { Handle, Position, type NodeProps } from '@xyflow/react';

import type { WorkflowCanvasNode } from '../../features/workflow/workflowModel';

/** 各节点类型在画布卡片顶部使用的渐变色。 */
const nodeAccent = {
  start: 'argus-accent-start',
  log: 'argus-accent-log',
  delay: 'argus-accent-delay',
  end: 'argus-accent-end',
};

/** 各节点类型在卡片中显示的简化图标。 */
const nodeIcon = {
  start: '▶',
  log: '≡',
  delay: '◷',
  end: '■',
};

/** 展示节点标签、运行状态、类型详情及可连接端点的画布卡片。 */
export function WorkflowNodeCard({ data, selected }: NodeProps<WorkflowCanvasNode>) {
  const details =
    data.kind === 'log'
      ? data.message
      : data.kind === 'delay'
        ? `${data.milliseconds ?? 0} ms`
        : data.kind === 'start'
          ? '工作流入口'
          : '工作流出口';
  const statusClass = {
    idle: 'argus-status-idle',
    running: 'argus-status-running',
    success: 'argus-status-success',
    error: 'argus-status-error',
  }[data.runState ?? 'idle'];

  return (
    <div
      className={`argus-node-card w-48 overflow-hidden border backdrop-blur transition ${
        data.invalid ? 'is-invalid' : selected ? 'is-selected' : ''
      }`}
    >
      {data.kind !== 'start' && (
        <Handle
          type="target"
          position={Position.Left}
          className="argus-handle !h-3 !w-3 !border-2"
        />
      )}
      <div className={`h-1 ${nodeAccent[data.kind]}`} />
      <div className="flex items-start gap-3 px-4 py-3.5">
        <span className="argus-node-icon flex h-9 w-9 shrink-0 items-center justify-center rounded-lg text-sm font-bold">
          {nodeIcon[data.kind]}
        </span>
        <div className="min-w-0 flex-1">
          <div className="flex items-center justify-between gap-3">
            <span className="argus-body text-sm font-bold">{data.label}</span>
            <span className={`h-2 w-2 rounded-full ${statusClass}`} />
          </div>
          <p className="argus-muted mt-1 truncate text-xs">{details}</p>
        </div>
      </div>
      {data.kind !== 'end' && (
        <Handle
          type="source"
          position={Position.Right}
          className="argus-handle !h-3 !w-3 !border-2"
        />
      )}
    </div>
  );
}
