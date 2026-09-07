import Circle from 'lucide-react/dist/esm/icons/circle.mjs';
import { useState } from 'react';
import type { RecorderController } from '../../features/recorder';
import { Button } from '../ui';
import { RecorderPanel } from './RecorderPanel';

/** 全局标题栏入口始终可见；首页、编辑器和面板切换都不卸载控制器。 */
export function RecorderToolbar({ recorder, workflowRunning }: Readonly<{
  recorder: RecorderController;
  workflowRunning: boolean;
}>) {
  const [open, setOpen] = useState(false);
  const recording = recorder.status.phase === 'recording';
  return (
    <>
      <Button
        size="compact"
        icon={Circle}
        variant={recording ? 'danger' : 'secondary'}
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={() => setOpen(true)}
      >
        {recorder.pending === 'stop' ? '正在保存…' : recording ? '录制中' : recorder.status.phase === 'awaiting_save' ? '录制待保存' : '录制操作'}
      </Button>
      <RecorderPanel
        open={open}
        onOpenChange={setOpen}
        recorder={recorder}
        workflowRunning={workflowRunning}
      />
    </>
  );
}
