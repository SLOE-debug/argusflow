import { useEffect, useState } from 'react';
import { recordingDate, type RecorderController } from '../../features/recorder';
import { Button, Dialog } from '../ui';
import { RecorderControls } from './RecorderControls';
import { RecordingTraceViewer } from './RecordingTraceViewer';

/** 录制操作面板，关闭面板保留后端监听及顶部状态入口。 */
export function RecorderPanel({ open, onOpenChange, recorder, workflowRunning }: Readonly<{
  open: boolean;
  onOpenChange: (open: boolean) => void;
  recorder: RecorderController;
  workflowRunning: boolean;
}>) {
  useEffect(() => { if (open) void recorder.refreshHistory(); }, [open, recorder.refreshHistory]);
  const [historyOpen, setHistoryOpen] = useState(false);
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="操作演示录制器"
      closeLabel="收起录制面板"
      size="fullscreen"
      className="select-text"
      compact
      headerActions={(
        <>
          {recorder.error ? (
            <span
              role="alert"
              title={recorder.error}
              className="max-w-80 truncate text-[11px] text-red-600"
            >{recorder.error}</span>
          ) : null}
          <Button
            size="compact"
            aria-expanded={historyOpen}
            onClick={() => setHistoryOpen((value) => !value)}
          >{historyOpen ? '收起历史' : '录制历史'}</Button>
          <RecorderControls recorder={recorder} workflowRunning={workflowRunning} />
        </>
      )}
    >
      <div className="flex min-h-0 flex-1 flex-col gap-2">
        <div className={`grid min-h-0 flex-1 gap-2 ${historyOpen ? 'grid-cols-[11rem_minmax(0,1fr)]' : 'grid-cols-[minmax(0,1fr)]'}`}>
          {historyOpen ? <aside className="flex min-h-0 flex-col rounded-lg border border-slate-200">
            <header className="flex items-center justify-between border-b border-slate-200 p-2">
              <h3 className="text-xs font-semibold">录制历史</h3>
              <Button
                size="compact"
                variant="ghost"
                onClick={() => void recorder.refreshHistory()}
              >刷新</Button>
            </header>
            <ul className="min-h-0 flex-1 overflow-y-auto p-1">
              {recorder.history.map((item) => (
                <li key={item.recording_id}>
                  <Button
                    size="compact"
                    variant={recorder.completed?.trace.recording_id === item.recording_id ? 'secondary' : 'ghost'}
                    className="h-auto w-full justify-start p-2 text-left"
                    disabled={recorder.pending !== null}
                    onClick={() => { void recorder.selectHistory(item.recording_id); setHistoryOpen(false); }}
                  >
                    <span className="block text-[11px]">
                      <span className="block">{recordingDate(item.started_at_unix_ms)}</span>
                      <span className="mt-1 block font-normal text-slate-500">{item.event_count} 条操作记录 · {item.screenshot_count} 份截图</span>
                    </span>
                  </Button>
                </li>
              ))}
              {recorder.history.length === 0 ? <li className="p-3 text-xs text-slate-500">暂无已保存的录制</li> : null}
            </ul>
          </aside> : null}
          {recorder.loadingHistory ? (
            <p
              role="status"
              className="p-5 text-sm text-slate-500"
            >正在读取录制…</p>
          ) : recorder.completed ? (
            <RecordingTraceViewer
              key={recorder.completed.trace.recording_id}
              recording={recorder.completed}
              onSaved={recorder.updateRecording}
            />
          ) : (
            <div className="flex items-center justify-center rounded-lg border border-dashed border-slate-300 p-8">
              <div className="max-w-md text-center">
                <h3 className="text-sm font-semibold">{recorder.status.phase === 'recording' ? '操作正在后台录制' : '记录一次完整的跨应用演示'}</h3>
                <p className="mt-2 text-xs leading-6 text-slate-500">
                  停止后，可按顺序查看操作和截图，检查哪些内容需要保留或删除。
                </p>
                <p className="mt-2 text-xs leading-6 text-slate-500">
                  录制会保留原始内容。分享前，可以删除不想公开的操作记录，也可以给截图打马赛克。部分中文输入可能未被记录，请留意检查。
                </p>
              </div>
            </div>
          )}
        </div>
        {recorder.status.phase === 'recording' ? <p className="text-[11px] text-slate-500">收起面板后会继续录制。完成后点击“停止并保存”。</p> : null}
      </div>
    </Dialog>
  );
}
