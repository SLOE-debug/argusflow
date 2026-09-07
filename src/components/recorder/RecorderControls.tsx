import Circle from 'lucide-react/dist/esm/icons/circle.mjs';
import Square from 'lucide-react/dist/esm/icons/square.mjs';
import { useEffect, useState } from 'react';
import { durationLabel, type RecorderController } from '../../features/recorder';
import { Button } from '../ui';

/** 全局监听控制与真实后端进度；面板关闭不影响生命周期。 */
export function RecorderControls({ recorder, workflowRunning }: Readonly<{
  recorder: RecorderController;
  workflowRunning: boolean;
}>) {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    if (recorder.status.phase !== 'recording') return;
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [recorder.status.phase]);
  const recording = recorder.status.phase === 'recording';
  const needsStop = recorder.status.phase !== 'idle';
  const stopping = recorder.pending === 'stop';
  const statusLabel = stopping ? '正在停止并保存…'
    : !recorder.available ? '请在 Windows 桌面应用中录制'
      : !recorder.ready ? '正在连接录制服务…'
        : recording ? '正在录制全局输入'
          : recorder.status.phase === 'awaiting_save' ? '录制已停止，等待保存'
            : recorder.status.phase === 'finishing' ? '录制已停止，等待整理'
              : recorder.completed ? '录制已保存' : '未开始录制';

  return (
    <section className="flex flex-wrap items-center gap-4 rounded-lg border border-slate-200 bg-slate-50 p-3">
      <div className="min-w-0 flex-1">
        <p className="flex items-center gap-2 text-sm font-semibold" role="status">
          <span
            aria-hidden="true"
            className={`size-2 rounded-full ${recording && !stopping ? 'animate-pulse bg-red-500' : 'bg-slate-400'}`}
          />
          {statusLabel}
          {recording && recorder.status.started_at_unix_ms !== null && !stopping ? (
            <span className="font-mono tabular-nums text-red-600">
              {durationLabel(now - recorder.status.started_at_unix_ms)}
            </span>
          ) : null}
        </p>
        <p className="mt-1 text-xs leading-5 text-slate-500">
          开始后，切换到目标应用并操作。返回这里停止，结果会自动保存。
        </p>
        {needsStop ? (
          <p className="text-xs leading-5 text-slate-600">
            已处理 {recorder.status.processed_events} 个原始事件 · 丢弃 {recorder.status.dropped_events} 个
          </p>
        ) : null}
        {workflowRunning ? <p className="text-xs text-amber-700">请先等待工作流运行结束，再开始录制。</p> : null}
      </div>
      <Button
        icon={needsStop ? Square : Circle}
        variant={needsStop ? 'danger' : 'primary'}
        loading={recorder.pending !== null}
        loadingLabel={stopping ? '正在停止并保存…' : '正在开始…'}
        disabled={!recorder.available || (!needsStop && (!recorder.ready || workflowRunning))}
        onClick={() => void (needsStop ? recorder.stop() : recorder.start())}
      >
        {needsStop ? recorder.status.phase === 'awaiting_save' ? '重试保存' : '停止并保存' : '开始录制'}
      </Button>
    </section>
  );
}
