import { useState } from 'react';
import { downloadTrace, durationLabel, eventLabel, eventContextLabel, sourceEventCount, traceJson, type CompletedRecording } from '../../features/recorder';
import { Button } from '../ui';
import { RecordingEventDetails } from './RecordingEventDetails';

/** 时间线按页浏览；JSON 导出始终包含完整事件和证据引用。 */
export function RecordingTraceViewer({ recording }: Readonly<{ recording: CompletedRecording }>) {
  const { trace } = recording;
  const [view, setView] = useState<'events' | 'json'>('events');
  const [selected, setSelected] = useState(0);
  const [page, setPage] = useState(0);
  const [notice, setNotice] = useState<string | null>(null);
  const [copying, setCopying] = useState(false);
  /** 每页 50 条整理后的事件，移动段占一行。 */
  const pageSize = 50;
  const events = trace.timeline.events;
  const sourceCount = sourceEventCount(trace);
  const pageCount = Math.max(1, Math.ceil(events.length / pageSize));
  const pageItems = events.slice(page * pageSize, (page + 1) * pageSize);
  const selectedEvent = events[selected];
  const copy = async () => {
    setCopying(true);
    try { await navigator.clipboard.writeText(traceJson(trace)); setNotice('已复制完整时间线。截图请从本地证据目录一起提供。'); }
    catch { setNotice('复制失败，请使用导出 JSON。'); }
    finally { setCopying(false); }
  };
  const exportJson = () => {
    try { downloadTrace(trace); setNotice('已请求下载时间线 JSON。截图保存在本地证据目录。'); }
    catch { setNotice('下载失败，可使用下方自动保存的本地文件。'); }
  };
  const changePage = (value: number) => { setPage(value); setSelected(value * pageSize); };
  return (
    <section className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden rounded-lg border border-slate-200">
      <header className="flex flex-wrap items-center gap-2 border-b border-slate-200 p-2">
        <nav
          className="flex gap-1"
          aria-label="录制查看方式"
        >
          {([{ id: 'events', label: '事件时间线' }, { id: 'json', label: '时间线 JSON' }] as const).map((tab) => (
            <Button
              key={tab.id}
              size="compact"
              variant={view === tab.id ? 'primary' : 'ghost'}
              aria-pressed={view === tab.id}
              onClick={() => { setView(tab.id); setNotice(null); }}
            >
              {tab.label}
            </Button>
          ))}
        </nav>
        <div className="ml-auto flex flex-wrap gap-2">
          <Button
            size="compact"
            loading={copying}
            onClick={() => void copy()}
          >复制时间线</Button>
          <Button
            size="compact"
            onClick={exportJson}
          >导出 JSON</Button>
        </div>
      </header>
      <div className="border-b border-slate-200 px-3 py-2 text-xs leading-5 text-slate-500">
        {events.length} 个事件 · 丢弃 {trace.dropped_events} 个
        {sourceCount > events.length ? <span> · 来自 {sourceCount} 个原始采样</span> : null}
        {notice ? <p role="status">{notice}</p> : null}
      </div>
      {view === 'events' ? (
        events.length === 0 ? <p className="p-5 text-sm text-slate-500">本次录制没有收到事件。</p> : (
          <div className="grid min-h-0 flex-1 grid-cols-[minmax(12rem,2fr)_minmax(0,3fr)]">
            <ol className="min-h-0 overflow-y-auto border-r border-slate-200 p-2">
              {pageItems.map((event, offset) => (
                <li key={event.sequence}>
                  <Button
                    variant={selected === page * pageSize + offset ? 'secondary' : 'ghost'}
                    className="h-auto w-full justify-start px-2 py-2 text-left"
                    aria-pressed={selected === page * pageSize + offset}
                    onClick={() => setSelected(page * pageSize + offset)}
                  >
                    <span className="block min-w-0">
                      <span className="block truncate">{page * pageSize + offset + 1}. {eventLabel(event.input)}</span>
                      <span className="mt-1 block truncate text-[11px] font-normal text-slate-500">
                        {durationLabel(event.elapsed_ms)} · {eventContextLabel(event)}</span>
                    </span>
                  </Button>
                </li>
              ))}
            </ol>
            <div className="min-h-0 overflow-y-auto">
              {selectedEvent ? (
                <RecordingEventDetails
                  event={selectedEvent}
                  recordingId={trace.recording_id}
                />
              ) : null}
            </div>
          </div>
        )
      ) : (
        <pre
          aria-label="时间线 JSON"
          className="min-h-0 flex-1 overflow-auto bg-slate-50 p-3 font-mono text-xs leading-5"
        >{JSON.stringify({ ...trace, timeline: { events: pageItems } }, null, 2)}</pre>
      )}
      <footer className="border-t border-slate-200 px-3 py-2 text-xs text-slate-500">
        {pageCount > 1 ? (
          <div className="mb-2 flex items-center justify-between gap-2">
            <Button
              size="compact"
              disabled={page === 0}
              onClick={() => changePage(page - 1)}
            >上一页</Button>
            <span>第 {page + 1}/{pageCount} 页 · 每页 {pageSize} 条</span>
            <Button
              size="compact"
              disabled={page + 1 >= pageCount}
              onClick={() => changePage(page + 1)}
            >下一页</Button>
          </div>
        ) : null}
        <details>
          <summary className="cursor-pointer">已自动保存完整演示包</summary>
          <p className="mt-1 break-all">时间线：{recording.files.timeline}</p>
          <p className="mt-1 break-all">截图目录：{recording.files.evidence_directory}</p>
          <p className="mt-1">提供给多模态 AI 时，请同时包含时间线、manifest 和截图目录。</p>
        </details>
      </footer>
    </section>
  );
}
