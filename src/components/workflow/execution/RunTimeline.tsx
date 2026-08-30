import ChevronRight from 'lucide-react/dist/esm/icons/chevron-right.mjs';

import type { RunTraceEvent } from '../../../features/workflow';
import { Button } from '../../ui';

type RunTimelineProps = Readonly<{
  traceEvents: ReadonlyArray<RunTraceEvent>;
  selectedSequence: number | null;
  onSelect: (event: RunTraceEvent) => void;
}>;

type TimelineGroup = Readonly<{
  key: string;
  label: string;
  events: ReadonlyArray<RunTraceEvent>;
}>;

/** 按节点折叠历史事件，避免把内部事件平铺成不可浏览的长日志。 */
export function RunTimeline({
  traceEvents,
  selectedSequence,
  onSelect,
}: RunTimelineProps) {
  const groups = groupTimelineEvents(traceEvents);

  return (
    <div className="min-h-0 overflow-y-auto px-2 py-1.5">
      <h3 className="mb-1.5 text-[10px] font-semibold text-slate-500">时间线</h3>
      {groups.length === 0 ? (
        <p className="text-[11px] text-slate-400">该运行还没有可读事件。</p>
      ) : (
        <div className="space-y-1">
          {groups.map((group) => (
            <details
              key={group.key}
              className="group rounded-md border border-slate-200 bg-white"
              open={group.events.some((item) => item.event.kind === 'node_failed')}
            >
              <summary className="flex cursor-pointer list-none items-center gap-1 px-2 py-1.5 text-[11px] font-semibold text-slate-700">
                <ChevronRight
                  className="size-3 text-slate-400 transition-transform group-open:rotate-90"
                  aria-hidden="true"
                />
                <span className="min-w-0 flex-1 truncate">{group.label}</span>
                <span className="text-[10px] font-normal text-slate-400">
                  {group.events.length}
                </span>
              </summary>
              <div className="border-t border-slate-100 px-1 py-1">
                {group.events.map((traceEvent) => (
                  <Button
                    key={traceEvent.trace_sequence}
                    variant={selectedSequence === traceEvent.trace_sequence ? 'secondary' : 'ghost'}
                    size="compact"
                    className="mb-0.5 w-full justify-start font-mono last:mb-0"
                    onClick={() => onSelect(traceEvent)}
                  >
                    <span className="w-7 text-right text-slate-400">
                      {traceEvent.event.sequence}
                    </span>
                    <span className="truncate">{eventLabel(traceEvent)}</span>
                  </Button>
                ))}
              </div>
            </details>
          ))}
        </div>
      )}
    </div>
  );
}

/** 节点 ID 是历史快照内的稳定分组键；工作流级事件单独成组。 */
function groupTimelineEvents(events: ReadonlyArray<RunTraceEvent>): TimelineGroup[] {
  const groups = new Map<string, RunTraceEvent[]>();
  for (const event of events) {
    const key = event.event.node_id ?? 'workflow';
    const group = groups.get(key) ?? [];
    group.push(event);
    groups.set(key, group);
  }
  return [...groups.entries()].map(([key, groupedEvents]) => ({
    key,
    label: key === 'workflow' ? '工作流' : key,
    events: groupedEvents,
  }));
}

function eventLabel(traceEvent: RunTraceEvent): string {
  return traceEvent.event.message ?? traceEvent.event.kind.replaceAll('_', ' ');
}
