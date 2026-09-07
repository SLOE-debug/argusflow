import { useEffect } from 'react';
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
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="语义录制器"
      description="记录点击、文字输入与快捷键。密码和无法确认的输入会自动遮盖。"
      closeLabel="收起录制面板"
      size="wide"
      className="select-text"
    >
      <div className="flex h-[min(76vh,52rem)] min-h-0 flex-col gap-3">
        <RecorderControls recorder={recorder} workflowRunning={workflowRunning} />
        {recorder.error ? (
          <p role="alert" className="rounded-md bg-red-50 px-3 py-2 text-xs text-red-700">{recorder.error}</p>
        ) : null}
        <div className="grid min-h-0 flex-1 grid-cols-[11rem_minmax(0,1fr)] gap-3">
          <aside className="flex min-h-0 flex-col rounded-lg border border-slate-200">
            <header className="flex items-center justify-between border-b border-slate-200 p-2">
              <h3 className="text-xs font-semibold">录制历史</h3>
              <Button size="compact" variant="ghost" onClick={() => void recorder.refreshHistory()}>刷新</Button>
            </header>
            <ul className="min-h-0 flex-1 overflow-y-auto p-1">
              {recorder.history.map((item) => (
                <li key={item.recording_id}>
                  <Button
                    size="compact"
                    variant={recorder.completed?.trace.recording_id === item.recording_id ? 'secondary' : 'ghost'}
                    className="h-auto w-full justify-start p-2 text-left"
                    disabled={recorder.pending !== null}
                    onClick={() => void recorder.selectHistory(item.recording_id)}
                  >
                    <span className="block text-[11px]">
                      <span className="block">{recordingDate(item.started_at_unix_ms)}</span>
                      <span className="mt-1 block font-normal text-slate-500">{item.semantic_record_count} 个步骤 · {item.raw_event_count} 个事件</span>
                    </span>
                  </Button>
                </li>
              ))}
              {recorder.history.length === 0 ? <li className="p-3 text-xs text-slate-500">暂无已保存的录制</li> : null}
            </ul>
          </aside>
          {recorder.loadingHistory ? (
            <p role="status" className="p-5 text-sm text-slate-500">正在读取录制…</p>
          ) : recorder.completed ? (
            <RecordingTraceViewer key={recorder.completed.trace.recording_id} recording={recorder.completed} />
          ) : (
            <div className="flex items-center justify-center rounded-lg border border-dashed border-slate-300 p-8">
              <div className="max-w-md text-center">
                <h3 className="text-sm font-semibold">{recorder.status.phase === 'recording' ? '操作正在后台录制' : '把实际操作记录为语义步骤'}</h3>
                <p className="mt-2 text-xs leading-6 text-slate-500">
                  停止并保存后，在这里查看操作、目标、定位候选和降级原因。语义 Trace 可以复制给 AI 整理流程。
                </p>
                <p className="mt-2 text-xs leading-6 text-slate-500">
                  当前支持普通键盘字符；中文输入法组合、拖拽和滚轮暂不生成完整语义步骤。
                </p>
              </div>
            </div>
          )}
        </div>
        <p className="text-[11px] text-slate-500">收起面板会继续录制。请使用“停止并保存”结束监听。</p>
      </div>
    </Dialog>
  );
}
