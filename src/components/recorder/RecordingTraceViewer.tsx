import { useState } from 'react';
import { BACKEND_LABELS, diagnosticLabel, downloadTrace, durationLabel, operationLabel,
  traceJson, type CompletedRecording, type TraceLayer } from '../../features/recorder';
import { Button } from '../ui';
import { RecordingStepDetails } from './RecordingStepDetails';

/** 分页渲染语义步骤与 Raw JSON，避免大量事件阻塞界面。 */
export function RecordingTraceViewer({ recording }: Readonly<{ recording: CompletedRecording }>) {
  const { trace } = recording;
  const [view, setView] = useState<'steps' | TraceLayer>('steps');
  const [selected, setSelected] = useState(0);
  const [page, setPage] = useState(0);
  const [notice, setNotice] = useState<string | null>(null);
  const [copying, setCopying] = useState(false);
  /** JSON 每页最多 50 条，导出始终包含完整层。 */
  const pageSize = 50;
  const items = view === 'raw' ? trace.raw.events : trace.normalized.records;
  const pageCount = Math.max(1, Math.ceil(items.length / pageSize));
  const pageItems = items.slice(page * pageSize, (page + 1) * pageSize);
  const layer = view === 'raw' ? 'raw' : 'normalized';
  const layerLabel = layer === 'raw' ? '原始 Trace' : '语义 Trace';
  const copy = async () => {
    setCopying(true);
    try { await navigator.clipboard.writeText(traceJson(trace, layer)); setNotice(`已复制完整${layerLabel}。`); }
    catch { setNotice('复制失败，请使用导出 JSON。'); }
    finally { setCopying(false); }
  };
  const exportJson = () => {
    try { downloadTrace(trace, layer); setNotice(`已请求下载完整${layerLabel}。`); }
    catch { setNotice('下载失败，可使用下方自动保存的本地文件。'); }
  };

  return (
    <section className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden rounded-lg border border-slate-200">
      <header className="flex flex-wrap items-center gap-2 border-b border-slate-200 p-2">
        <nav className="flex gap-1" aria-label="录制查看方式">
          {([{ id: 'steps', label: '操作步骤' }, { id: 'normalized', label: '语义 Trace' },
            { id: 'raw', label: '原始 Trace' }] as const).map((tab) => (
            <Button
              key={tab.id}
              size="compact"
              variant={view === tab.id ? 'primary' : 'ghost'}
              aria-pressed={view === tab.id}
              onClick={() => { setView(tab.id); setPage(0); setNotice(null); }}
            >
              {tab.label}
            </Button>
          ))}
        </nav>
        <div className="ml-auto flex flex-wrap gap-2">
          <Button size="compact" loading={copying} onClick={() => void copy()}>复制{layerLabel}</Button>
          <Button size="compact" onClick={exportJson}>导出 JSON</Button>
        </div>
      </header>
      <div className="border-b border-slate-200 px-3 py-2 text-xs leading-5 text-slate-500">
        {trace.normalized.records.length} 个步骤 · {trace.raw.events.length} 个原始事件 · 丢弃 {trace.dropped_events} 个
        {notice ? <p role="status" className="text-blue-700">{notice}</p> : null}
        {trace.normalized.diagnostics.map((item, index) => (
          <p key={index} className="text-amber-800">{diagnosticLabel(item)}</p>
        ))}
      </div>
      {view === 'steps' ? (
        trace.normalized.records.length === 0 ? (
          <p className="p-5 text-sm text-slate-500">没有可归一化的操作。可查看原始 Trace 和诊断。</p>
        ) : (
          <div className="grid min-h-0 flex-1 grid-cols-[minmax(12rem,2fr)_minmax(0,3fr)]">
            <ol className="min-h-0 overflow-y-auto border-r border-slate-200 p-2">
              {trace.normalized.records.slice(page * pageSize, (page + 1) * pageSize).map((record, offset) => (
                <li key={record.sequence} className="mb-1">
                  <Button
                    variant={selected === page * pageSize + offset ? 'secondary' : 'ghost'}
                    className="h-auto w-full justify-start px-2 py-2 text-left"
                    aria-pressed={selected === page * pageSize + offset}
                    onClick={() => setSelected(page * pageSize + offset)}
                  >
                    <span className="block min-w-0">
                      <span className="block truncate">{record.sequence}. {operationLabel(record.operation)}</span>
                      <span className="mt-1 block truncate text-[11px] font-normal text-slate-500">
                        {durationLabel(record.started_ms)} · {BACKEND_LABELS[record.target.backend]} · {record.target.entity?.semantics.name ?? record.target.context?.title ?? '未知目标'}
                      </span>
                    </span>
                  </Button>
                </li>
              ))}
            </ol>
            <div className="min-h-0 overflow-y-auto">
              {trace.normalized.records[selected] ? <RecordingStepDetails record={trace.normalized.records[selected]} /> : null}
            </div>
          </div>
        )
      ) : (
        <pre
          aria-label={layerLabel}
          className="min-h-0 flex-1 overflow-auto bg-slate-50 p-3 font-mono text-xs leading-5"
        >
          {JSON.stringify(view === 'raw' ? { events: pageItems }
            : { records: pageItems, diagnostics: trace.normalized.diagnostics }, null, 2)}
        </pre>
      )}
      <footer className="border-t border-slate-200 px-3 py-2 text-xs text-slate-500">
        {pageCount > 1 ? (
          <div className="mb-2 flex items-center justify-between gap-2">
            <Button size="compact" disabled={page === 0} onClick={() => setPage(page - 1)}>上一页</Button>
            <span>第 {page + 1}/{pageCount} 页 · 每页 {pageSize} 条，复制与导出包含全部记录</span>
            <Button size="compact" disabled={page + 1 >= pageCount} onClick={() => setPage(page + 1)}>下一页</Button>
          </div>
        ) : null}
        <details>
          <summary className="cursor-pointer">已自动保存到本地</summary>
          <p className="mt-1 break-all">语义：{recording.files.normalized}</p>
          <p className="mt-1 break-all">原始：{recording.files.raw}</p>
        </details>
      </footer>
    </section>
  );
}
