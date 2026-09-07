import { useEffect, useState } from 'react';
import { readRecordingScreenshot, type ScreenshotEvidence } from '../../features/recorder';

/** 图像读取状态；URL 只在当前事件预览生命周期内有效。 */
type ImageState = Readonly<{ type: 'loading' }> | Readonly<{ type: 'failed' }> | Readonly<{ type: 'ready'; url: string }>;

/** 通过受限 IPC 显示 PNG，不使用任意文件协议。 */
function EvidenceImage({ recordingId, sequence, kind }: Readonly<{ recordingId: string; sequence: number; kind: 'window' | 'crop' }>) {
  const [state, setState] = useState<ImageState>({ type: 'loading' });
  useEffect(() => {
    let cancelled = false;
    let imageUrl: string | undefined;
    setState({ type: 'loading' });
    void readRecordingScreenshot(recordingId, sequence, kind).then((bytes) => {
      if (cancelled) return;
      imageUrl = URL.createObjectURL(new Blob([bytes], { type: 'image/png' }));
      setState({ type: 'ready', url: imageUrl });
    }).catch(() => { if (!cancelled) setState({ type: 'failed' }); });
    return () => { cancelled = true; if (imageUrl) URL.revokeObjectURL(imageUrl); };
  }, [recordingId, sequence, kind]);
  if (state.type === 'loading') return <p className="text-xs text-slate-500">正在读取截图…</p>;
  if (state.type === 'failed') return <p className="text-xs text-amber-800">截图读取失败，请检查本地证据文件。</p>;
  return (
    <img
      src={state.url}
      alt={kind === 'window' ? '事件发生时的可见窗口区域' : '鼠标按下位置附近的局部截图'}
      className="max-h-96 max-w-full rounded border border-slate-200 object-contain"
    />
  );
}

/** 完整图像与点击局部证据共享事件身份，坐标使用物理屏幕单位。 */
export function ScreenshotPreview({ recordingId, sequence, screenshot }: Readonly<{
  recordingId: string; sequence: number; screenshot: ScreenshotEvidence;
}>) {
  return (
    <section className="space-y-2">
      <h4 className="text-xs font-semibold">截图证据</h4>
      {screenshot.crop_failure ? <p className="text-xs text-amber-800">局部截图保存失败，完整窗口截图已保留。</p> : null}
      <EvidenceImage
        key={`${sequence}-window`}
        recordingId={recordingId}
        sequence={sequence}
        kind="window"
      />
      {screenshot.pointer ? <p className="text-xs text-slate-500">鼠标屏幕坐标：({screenshot.pointer.x}, {screenshot.pointer.y})</p> : null}
      {screenshot.crop ? (
        <EvidenceImage
          key={`${sequence}-crop`}
          recordingId={recordingId}
          sequence={sequence}
          kind="crop"
        />
      ) : null}
    </section>
  );
}
