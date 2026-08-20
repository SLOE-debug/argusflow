import { CircleCheck } from 'lucide-react';

import type {
  ExecutionEvent,
  ValidationReport,
} from '../../features/workflow/contracts';

type ExecutionLogProps = {
  /** 按后端 sequence 顺序排列的实时执行事件。 */
  events: ExecutionEvent[];
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
  return (
    <section
      className={
        'grid h-[152px] min-h-0 grid-cols-[minmax(0,1fr)_340px] ' +
        'border-t border-slate-200'
      }
    >
      <div className="min-w-0 overflow-auto px-3 py-2.5">
        <div className="mb-2 flex items-center justify-between">
          <h2 className="text-[11px] font-bold tracking-[0.1em] text-slate-500 uppercase">
            执行日志
          </h2>
          <span className="text-[11px] text-slate-400">{events.length} events</span>
        </div>
        <div className="h-24 space-y-1 overflow-y-auto font-mono text-[11px] leading-5">
          {events.length === 0 && (
            <p className="text-slate-400">运行工作流后，事件会显示在这里。</p>
          )}
          {events.map((event) => (
            <div
              key={`${event.run_id}-${event.sequence}`}
              className="flex gap-2.5"
            >
              <span className="w-7 shrink-0 text-right text-slate-400">
                {String(event.sequence).padStart(2, '0')}
              </span>
              <span className={`w-[136px] shrink-0 ${eventTone[event.kind]}`}>
                {event.kind}
              </span>
              <span className="truncate text-slate-600">
                {event.node_id ? `[${event.node_id}] ` : ''}
                {event.message ?? ''}
              </span>
            </div>
          ))}
        </div>
      </div>
      <div className="min-w-0 overflow-auto border-l border-slate-200 px-3 py-2.5">
        <h2 className="mb-2 text-[11px] font-bold tracking-[0.1em] text-slate-500 uppercase">
          校验结果
        </h2>
        <div className="h-24 overflow-y-auto text-xs leading-5">
          {!report && <p className="text-slate-400">尚未校验</p>}
          {report?.valid && (
            <p className="flex items-center gap-1.5 text-emerald-700">
              <CircleCheck
                className="size-5 shrink-0"
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
