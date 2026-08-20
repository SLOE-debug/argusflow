import type { ExecutionEvent, ValidationReport } from '../../features/workflow/contracts';

type ExecutionLogProps = {
  /** 按后端 sequence 顺序排列的实时执行事件。 */
  events: ExecutionEvent[];
  /** 最近一次工作流结构校验结果。 */
  report: ValidationReport | null;
};

/** 为不同事件类型配置日志文字颜色，便于区分生命周期和错误。 */
const eventTone = {
  workflow_started: 'argus-event-info',
  node_started: 'argus-event-neutral',
  log: 'argus-event-log',
  node_succeeded: 'argus-event-success',
  node_failed: 'argus-event-error',
  workflow_completed: 'argus-event-success',
  workflow_failed: 'argus-event-error',
};

/** 展示执行事件流及结构校验问题。 */
export function ExecutionLog({ events, report }: ExecutionLogProps) {
  return (
    <section className="argus-log-panel grid min-h-44 grid-cols-[minmax(0,1fr)_340px] border-t">
      <div className="min-w-0 p-5">
        <div className="mb-3 flex items-center justify-between">
          <h2 className="argus-muted text-xs font-bold tracking-[0.12em] uppercase">
            执行日志
          </h2>
          <span className="argus-subtle text-xs">{events.length} events</span>
        </div>
        <div className="argus-mono h-24 space-y-1 overflow-y-auto text-xs leading-5">
          {events.length === 0 && (
            <p className="argus-subtle">运行工作流后，事件会显示在这里。</p>
          )}
          {events.map((event) => (
            <div key={`${event.run_id}-${event.sequence}`} className="flex gap-3">
              <span className="argus-subtle w-7 shrink-0 text-right">
                {String(event.sequence).padStart(2, '0')}
              </span>
              <span className={`w-34 shrink-0 ${eventTone[event.kind]}`}>{event.kind}</span>
              <span className="argus-muted truncate">
                {event.node_id ? `[${event.node_id}] ` : ''}
                {event.message ?? ''}
              </span>
            </div>
          ))}
        </div>
      </div>
      <div className="argus-divider border-l p-5">
        <h2 className="argus-muted mb-3 text-xs font-bold tracking-[0.12em] uppercase">
          校验结果
        </h2>
        <div className="h-24 overflow-y-auto text-xs leading-5">
          {!report && <p className="argus-subtle">尚未校验</p>}
          {report?.valid && <p className="argus-event-success">✓ 工作流结构有效</p>}
          {report?.issues.map((issue, index) => (
            <p key={`${issue.code}-${issue.node_id}-${index}`} className="argus-event-error mb-1">
              {issue.node_id ? `[${issue.node_id}] ` : ''}
              {issue.message}
            </p>
          ))}
        </div>
      </div>
    </section>
  );
}
