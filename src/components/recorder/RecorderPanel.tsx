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
      title="操作演示录制器"
      description="记录鼠标、键盘、窗口与剪贴板变化，并保存当时的界面证据。"
      closeLabel="收起录制面板"
      size="wide"
      className="select-text"
    >
      <div className="flex h-[min(76vh,52rem)] min-h-0 flex-col gap-3">
        <RecorderControls
          recorder={recorder}
          workflowRunning={workflowRunning}
        />
        {recorder.error ? (
          <p
            role="alert"
            className="rounded-md bg-red-50 px-3 py-2 text-xs text-red-700"
          >{recorder.error}</p>
        ) : null}
        <div className="grid min-h-0 flex-1 grid-cols-[11rem_minmax(0,1fr)] gap-3">
          <aside className="flex min-h-0 flex-col rounded-lg border border-slate-200">
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
                    onClick={() => void recorder.selectHistory(item.recording_id)}
                  >
                    <span className="block text-[11px]">
                      <span className="block">{recordingDate(item.started_at_unix_ms)}</span>
                      <span className="mt-1 block font-normal text-slate-500">{item.event_count} 个事件 · {item.screenshot_count} 份截图</span>
                    </span>
                  </Button>
                </li>
              ))}
              {recorder.history.length === 0 ? <li className="p-3 text-xs text-slate-500">暂无已保存的录制</li> : null}
            </ul>
          </aside>
          {recorder.loadingHistory ? (
            <p
              role="status"
              className="p-5 text-sm text-slate-500"
            >正在读取录制…</p>
          ) : recorder.completed ? (
            <RecordingTraceViewer
              key={recorder.completed.trace.recording_id}
              recording={recorder.completed}
            />
          ) : (
            <div className="flex items-center justify-center rounded-lg border border-dashed border-slate-300 p-8">
              <div className="max-w-md text-center">
                <h3 className="text-sm font-semibold">{recorder.status.phase === 'recording' ? '操作正在后台录制' : '记录一次完整的跨应用演示'}</h3>
                <p className="mt-2 text-xs leading-6 text-slate-500">
                  停止后可查看事件时间线、界面快照和截图。将完整演示包交给多模态 AI，理解任务后再生成工作流。
                </p>
                <p className="mt-2 text-xs leading-6 text-slate-500">
                  键盘敏感内容会遮盖；截图和剪贴板文本按实际内容保存。中文输入法最终提交文字仍可能缺失。
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
