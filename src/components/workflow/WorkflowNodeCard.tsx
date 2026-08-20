import { Handle, Position, type NodeProps } from '@xyflow/react';

import type { WorkflowCanvasNode } from '../../features/workflow/workflowModel';

/** 各节点类型在画布卡片顶部使用的渐变色。 */
const nodeAccent = {
  start: 'from-emerald-400 to-teal-500',
  log: 'from-sky-400 to-blue-500',
  delay: 'from-amber-300 to-orange-500',
  end: 'from-violet-400 to-fuchsia-500',
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
    idle: 'bg-slate-500',
    running: 'bg-sky-400 shadow-[0_0_12px_#38bdf8]',
    success: 'bg-emerald-400',
    error: 'bg-rose-400 shadow-[0_0_12px_#fb7185]',
  }[data.runState ?? 'idle'];

  return (
    <div
      className={`w-46 overflow-hidden rounded-xl border bg-[#101f33]/95 shadow-xl backdrop-blur transition ${
        data.invalid
          ? 'border-rose-400 ring-2 ring-rose-400/20'
          : selected
            ? 'border-sky-400 ring-2 ring-sky-400/20'
            : 'border-[#29415e]'
      }`}
    >
      {data.kind !== 'start' && (
        <Handle
          type="target"
          position={Position.Left}
          className="!h-3 !w-3 !border-2 !border-[#0b1627] !bg-sky-400"
        />
      )}
      <div className={`h-1 bg-gradient-to-r ${nodeAccent[data.kind]}`} />
      <div className="flex items-start gap-3 px-4 py-3">
        <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-white/8 text-sm text-white">
          {nodeIcon[data.kind]}
        </span>
        <div className="min-w-0 flex-1">
          <div className="flex items-center justify-between gap-3">
            <span className="text-sm font-semibold text-slate-100">{data.label}</span>
            <span className={`h-2 w-2 rounded-full ${statusClass}`} />
          </div>
          <p className="mt-1 truncate text-xs text-slate-400">{details}</p>
        </div>
      </div>
      {data.kind !== 'end' && (
        <Handle
          type="source"
          position={Position.Right}
          className="!h-3 !w-3 !border-2 !border-[#0b1627] !bg-sky-400"
        />
      )}
    </div>
  );
}
