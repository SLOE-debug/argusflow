import { useState } from 'react';
import { useEvidenceImage } from '../../features/recorder';
import { Button } from '../ui';

/** 按已保存帧引用逐张查看，不把下一事件的像素重新归到当前事件。 */
export function EventScreenFrames({ recordingId, before, after }: Readonly<{
  recordingId: string;
  before: readonly number[];
  after: readonly number[];
}>) {
  const frames = [...before, ...after];
  const [position, setPosition] = useState(0);
  const index = Math.min(position, Math.max(0, frames.length - 1));
  const frameId = frames[index];
  return frameId === undefined ? null : (
    <section className="space-y-2" aria-label="事件关联画面">
      <div className="flex items-center gap-2">
        <Button disabled={index === 0} onClick={() => setPosition(index - 1)}>上一帧</Button>
        <span className="text-xs text-slate-600">{index < before.length ? '操作前' : '后续变化'} · 帧 {frameId} · {index + 1}/{frames.length}</span>
        <Button disabled={index + 1 >= frames.length} onClick={() => setPosition(index + 1)}>下一帧</Button>
      </div>
      <FrameImage recordingId={recordingId} frameId={frameId} />
      <p className="text-xs text-slate-500">画面按时间关联，不表示变化由该操作引起。</p>
    </section>
  );
}

function FrameImage({ recordingId, frameId }: Readonly<{ recordingId: string; frameId: number }>) {
  const image = useEvidenceImage(recordingId, frameId, 'screen');
  return image.type === 'ready'
    ? <img src={image.url} alt={`录制屏幕帧 ${frameId}`} className="max-h-96 max-w-full rounded border border-slate-200 object-contain" />
    : <p role="status" className="text-xs text-slate-600">{image.type === 'loading' ? '正在读取画面…' : '画面读取失败，请重新打开录制。'}</p>;
}
