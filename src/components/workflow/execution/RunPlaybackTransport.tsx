import ChevronLeft from 'lucide-react/dist/esm/icons/chevron-left.mjs';
import ChevronRight from 'lucide-react/dist/esm/icons/chevron-right.mjs';
import RadioTower from 'lucide-react/dist/esm/icons/radio-tower.mjs';

import {
  resolveSnapshotExecutionLogEntry,
  type ExecutionEvent,
  type RunPresentationSnapshot,
} from '../../../features/workflow';
import { Button, IconButton } from '../../ui';
import { RunEventScale } from './RunEventScale';

type RunPlaybackTransportProps = Readonly<{
  events: ReadonlyArray<ExecutionEvent>;
  cursor: number;
  presentation: RunPresentationSnapshot | null;
  followLatest: boolean;
  currentSource: boolean;
  onCursorChange: (cursor: number) => void;
  onReturnToLatest: () => void;
}>;

/** 将事件摘要与居中的刻度回放轴组合为执行台底部传输控件。 */
export function RunPlaybackTransport({
  events,
  cursor,
  presentation,
  followLatest,
  currentSource,
  onCursorChange,
  onReturnToLatest,
}: RunPlaybackTransportProps) {
  const selectedEvent = cursor >= 0 ? events[cursor] ?? null : null;
  const summary = selectedEvent
    ? resolveSnapshotExecutionLogEntry(selectedEvent, presentation?.node_labels ?? {})
    : null;
  const valueText = summary ? `${summary.nodeLabel ?? '工作流'}：${summary.eventLabel}` : '没有事件';
  const summaryText = summary
    ? `${summary.nodeLabel ?? '工作流'} · ${summary.eventLabel}${summary.detail ? ` — ${summary.detail}` : ''}`
    : '运行开始后，当前事件会显示在这里。';
  return (
    <footer className="grid h-[112px] shrink-0 grid-cols-[36px_minmax(0,1fr)_36px] items-center gap-3 border-t border-slate-200 bg-white px-4 py-2">
      <IconButton
        icon={ChevronLeft}
        label="上一个事件"
        className="justify-self-start"
        disabled={cursor <= 0}
        onClick={() => onCursorChange(cursor - 1)}
      />
      <div className="min-w-0 self-stretch">
        <div className="grid h-12 grid-cols-[minmax(112px,1fr)_minmax(0,3fr)_minmax(112px,1fr)] items-center gap-3">
          <div className="flex items-center gap-2 text-[12px] font-medium text-slate-500">
            <span>事件 {events.length === 0 ? 0 : cursor + 1} / {events.length}</span>
            {currentSource && followLatest ? (
              <span className="inline-flex items-center gap-1 whitespace-nowrap text-emerald-600">
                <span className="size-2 rounded-full bg-emerald-500" />实时跟随
              </span>
            ) : null}
          </div>
          <p className="truncate text-center text-[13px] font-medium text-slate-800" title={summaryText}>
            {summaryText}
          </p>
          <div className="flex justify-end">
            {currentSource && !followLatest && events.length > 0 ? (
              <Button icon={RadioTower} size="compact" onClick={onReturnToLatest}>
                回到最新
              </Button>
            ) : null}
          </div>
        </div>
        <RunEventScale
          events={events}
          cursor={cursor}
          valueText={valueText}
          onCursorChange={onCursorChange}
        />
      </div>
      <IconButton
        icon={ChevronRight}
        label="下一个事件"
        className="justify-self-end"
        disabled={cursor < 0 || cursor >= events.length - 1}
        onClick={() => onCursorChange(cursor + 1)}
      />
    </footer>
  );
}
