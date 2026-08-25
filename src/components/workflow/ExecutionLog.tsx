import { CircleCheck, Copy } from 'lucide-react';

import type {
  ExecutionEvent,
  ValidationReport,
} from '../../features/workflow/contracts';

type ExecutionLogProps = {
  /** 按后端 sequence 顺序排列的实时执行事件。 */
  events: ReadonlyArray<ExecutionEvent>;
  /** 最近一次工作流结构校验结果。 */
  report: ValidationReport | null;
};

/** 为不同事件类型配置日志文字颜色，便于区分生命周期和错误。 */
const eventTone = {
  workflow_started: 'text-blue-600',
  node_started: 'text-slate-500',
  log: 'text-teal-700',
  node_succeeded: 'text-emerald-700',
  edge_traversed: 'text-blue-600',
  node_failed: 'text-rose-700',
  workflow_completed: 'text-emerald-700',
  workflow_failed: 'text-rose-700',
};

/** 展示执行事件流及结构校验问题。 */
export function ExecutionLog({ events, report }: ExecutionLogProps) {
  /** 复制时保留序号、事件类别、节点和完整消息，便于直接提交故障信息。 */
  const completeLog = events.map(formatEvent).join('\n');

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
            <span className="text-[11px] text-slate-400">{events.length} events</span>
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
          {events.map((event) => (
            <div
              key={`${event.run_id}-${event.sequence}`}
              className="flex items-start gap-2"
            >
              <span className="w-6 shrink-0 text-right text-slate-400">
                {String(event.sequence).padStart(2, '0')}
              </span>
              <span className={`w-[124px] shrink-0 ${eventTone[event.kind]}`}>
                {event.kind}
              </span>
              <span className="min-w-0 select-text whitespace-pre-wrap break-all text-slate-600">
                {event.node_id ? `[${event.node_id}] ` : ''}
                {event.message ?? ''}
              </span>
            </div>
          ))}
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

/** 把单条事件格式化为不丢字段的可复制文本行。 */
function formatEvent(event: ExecutionEvent): string {
  const sequence = String(event.sequence).padStart(2, '0');
  const node = event.node_id ? `[${event.node_id}] ` : '';
  return `${sequence} ${event.kind} ${node}${event.message ?? ''}`;
}
