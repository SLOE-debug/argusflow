import { CircleCheck, Copy } from 'lucide-react';

import type { ExecutionEvent, ValidationReport } from '../../features/workflow/contracts';
import {
  formatExecutionLogEntry,
  resolveExecutionLogEntry,
  type ExecutionLogSeverity,
} from '../../features/workflow/executionEventPresentation';
import type {
  WorkflowCanvasNode,
  WorkflowNodeData,
} from '../../features/workflow/workflowModel';

type ExecutionLogProps = {
  /** 按后端 sequence 顺序排列的实时执行事件。 */
  events: ReadonlyArray<ExecutionEvent>;
  /** 当前工作流节点，用于把协议 node_id 解析成用户名称和节点身份色。 */
  nodes: ReadonlyArray<WorkflowCanvasNode>;
  /** 最近一次工作流结构校验结果。 */
  report: ValidationReport | null;
};

/** 节点类型与画布左侧强调色保持一致，形成跨视图身份提示。 */
const NODE_LOG_TONES = {
  start: 'border-emerald-500 text-emerald-700',
  log: 'border-blue-500 text-blue-700',
  debug: 'border-fuchsia-500 text-fuchsia-700',
  delay: 'border-amber-500 text-amber-700',
  condition: 'border-violet-500 text-violet-700',
  variable: 'border-teal-500 text-teal-700',
  application: 'border-indigo-500 text-indigo-700',
  browser: 'border-sky-500 text-sky-700',
  navigate: 'border-sky-500 text-sky-700',
  ui: 'border-cyan-500 text-cyan-700',
  command: 'border-slate-600 text-slate-700',
  format: 'border-amber-500 text-amber-700',
  component: 'border-violet-600 text-violet-700',
  end: 'border-rose-500 text-rose-700',
} satisfies Readonly<Record<WorkflowNodeData['kind'], string>>;

/** 事件状态颜色只表达成功、警告和失败，不覆盖节点身份色。 */
const EVENT_SEVERITY_TONES = {
  normal: 'text-slate-600',
  success: 'text-emerald-700',
  warning: 'text-amber-700',
  error: 'font-semibold text-rose-700',
} satisfies Readonly<Record<ExecutionLogSeverity, string>>;

/** 展示执行事件流及结构校验问题。 */
export function ExecutionLog({ events, nodes, report }: ExecutionLogProps) {
  const entries = events.map((event) => resolveExecutionLogEntry(event, nodes));
  /** 默认复制本地化可读日志；原始协议仍保留在事件状态中供后续开发者模式使用。 */
  const completeLog = entries.map(formatExecutionLogEntry).join('\n');

  return (
    <section
      className={
        'grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_300px] ' +
        'border-t border-slate-200'
      }
    >
      <div className="min-w-0 overflow-auto px-2 py-1.5">
        <div className="mb-1 flex items-center justify-between">
          <h2 className="text-[10px] font-semibold text-slate-500">
            执行日志
          </h2>
          <div className="flex items-center gap-1.5">
            <span className="text-[11px] text-slate-400">{events.length} 条事件</span>
            <button
              type="button"
              aria-label="复制完整执行日志"
              className="flex size-5 items-center justify-center rounded text-slate-400 hover:bg-slate-100 hover:text-slate-700 disabled:opacity-40"
              disabled={events.length === 0}
              onClick={() => void navigator.clipboard.writeText(completeLog)}
              title="复制完整执行日志"
            >
              <Copy className="size-3" aria-hidden="true" />
            </button>
          </div>
        </div>
        <div className="space-y-0.5 overflow-y-auto font-mono text-[10px] leading-4">
          {events.length === 0 && (
            <p className="text-slate-400">运行工作流后，事件会显示在这里。</p>
          )}
          {entries.map((entry) => {
            const nodeTone = entry.nodeKind
              ? NODE_LOG_TONES[entry.nodeKind]
              : 'border-blue-400 text-blue-700';
            return (
              <div
                key={entry.sequence}
                className={`grid grid-cols-[24px_minmax(92px,124px)_72px_minmax(0,1fr)] items-start gap-2 border-l-2 pl-1.5 ${nodeTone}`}
                data-node-tone={entry.nodeKind ?? 'workflow'}
              >
                <span className="w-6 shrink-0 text-right text-slate-400">
                  {String(entry.sequence).padStart(2, '0')}
                </span>
                <span
                  className="truncate font-sans font-medium"
                  title={entry.nodeId ? `节点 ID：${entry.nodeId}` : undefined}
                >
                  {entry.nodeLabel ?? (entry.nodeId ? `节点 ${entry.nodeId}` : '工作流')}
                </span>
                <span className={EVENT_SEVERITY_TONES[entry.severity]}>
                  {entry.eventLabel}
                </span>
                <span className="min-w-0 select-text whitespace-pre-wrap break-all text-slate-600">
                  {entry.detail}
                </span>
              </div>
            );
          })}
        </div>
      </div>
      <div className="min-w-0 overflow-auto border-l border-slate-200 px-2 py-1.5">
        <h2 className="mb-1 text-[10px] font-semibold text-slate-500">
          校验结果
        </h2>
        <div className="overflow-y-auto text-[11px] leading-4">
          {!report && <p className="text-slate-400">尚未校验</p>}
          {report?.valid && (
            <p className="flex items-center gap-1.5 text-emerald-700">
              <CircleCheck
                className="size-3.5 shrink-0"
                aria-hidden="true"
              />
              工作流结构有效
            </p>
          )}
          {report?.issues.map((issue, index) => (
            <p key={`${issue.code}-${issue.node_id}-${index}`} className="mb-1 text-rose-700">
              {issue.node_id ? `[${issue.node_id}] ` : ''}
              {issue.message}
            </p>
          ))}
        </div>
      </div>
    </section>
  );
}
