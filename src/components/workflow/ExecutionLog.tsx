import type { ExecutionEvent, ValidationReport } from '../../features/workflow/contracts';

type ExecutionLogProps = {
  /** 按后端 sequence 顺序排列的实时执行事件。 */
  events: ExecutionEvent[];
  /** 最近一次工作流结构校验结果。 */
  report: ValidationReport | null;
};

/** 为不同事件类型配置日志文字颜色，便于区分生命周期和错误。 */
const eventTone = {
  workflow_started: 'text-sky-300',
  node_started: 'text-slate-400',
  log: 'text-cyan-200',
  node_succeeded: 'text-emerald-300',
  node_failed: 'text-rose-300',
  workflow_completed: 'text-emerald-300',
  workflow_failed: 'text-rose-300',
};

/** 展示执行事件流及结构校验问题。 */
export function ExecutionLog({ events, report }: ExecutionLogProps) {
  return (
    <section className="grid min-h-40 grid-cols-[minmax(0,1fr)_320px] border-t border-[#1d3048] bg-[#081321]">
      <div className="min-w-0 p-4">
        <div className="mb-3 flex items-center justify-between">
          <h2 className="text-xs font-semibold tracking-[0.14em] text-slate-400 uppercase">
            执行日志
          </h2>
          <span className="text-[10px] text-slate-600">{events.length} events</span>
        </div>
        <div className="h-24 space-y-1 overflow-y-auto font-mono text-[11px]">
          {events.length === 0 && <p className="text-slate-600">运行工作流后，事件会显示在这里。</p>}
          {events.map((event) => (
            <div key={`${event.run_id}-${event.sequence}`} className="flex gap-3">
              <span className="w-7 shrink-0 text-right text-slate-700">
                {String(event.sequence).padStart(2, '0')}
              </span>
              <span className={`w-34 shrink-0 ${eventTone[event.kind]}`}>{event.kind}</span>
              <span className="truncate text-slate-400">
                {event.node_id ? `[${event.node_id}] ` : ''}
                {event.message ?? ''}
              </span>
            </div>
          ))}
        </div>
      </div>
      <div className="border-l border-[#1d3048] p-4">
        <h2 className="mb-3 text-xs font-semibold tracking-[0.14em] text-slate-400 uppercase">
          校验结果
        </h2>
        <div className="h-24 overflow-y-auto text-[11px]">
          {!report && <p className="text-slate-600">尚未校验</p>}
          {report?.valid && <p className="text-emerald-300">✓ 工作流结构有效</p>}
          {report?.issues.map((issue, index) => (
            <p key={`${issue.code}-${issue.node_id}-${index}`} className="mb-1 text-rose-300">
              {issue.node_id ? `[${issue.node_id}] ` : ''}
              {issue.message}
            </p>
          ))}
        </div>
      </div>
    </section>
  );
}
