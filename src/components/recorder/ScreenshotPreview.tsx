import { useEvidenceImage, type ScreenshotEvidence } from '../../features/recorder';
import { ClickEvidenceMarker } from './ClickEvidenceMarker';

/** 完整图与局部图的像素展示，数据读取由专用 Hook 管理。 */
function EvidenceImage({ recordingId, sequence, kind, screenshot }: Readonly<{ recordingId: string; sequence: number; kind: 'window' | 'crop' | 'target' | 'target_crop'; screenshot?: ScreenshotEvidence }>) {
  const state = useEvidenceImage(recordingId, sequence, kind);
  /** 放大图放在远离点击的水平角落，避免遮住定位框。 */
  const insetSide = screenshot?.pointer && screenshot.pointer.x - screenshot.screen_bounds.x > screenshot.width / 2
    ? 'left-2' : 'right-2';
  if (state.type === 'loading') return <p className="text-xs text-slate-500">正在读取截图…</p>;
  if (state.type === 'failed') return <p className="text-xs text-amber-800">截图读取失败，请检查本地证据文件。</p>;
  return (
    <div className="relative inline-block max-w-full align-top">
      <img
        src={state.url}
        alt={kind === 'window' ? '操作后的可见窗口区域' : kind === 'target' ? '点击时的目标窗口区域' : '鼠标按下位置附近的局部截图'}
        className="max-h-96 max-w-full rounded object-contain"
      />
      {screenshot ? <ClickEvidenceMarker screenshot={screenshot} /> : null}
      {screenshot?.crop ? (
        <figure className={`absolute bottom-2 ${insetSide} max-h-[45%] max-w-[35%] overflow-hidden rounded border-2 border-white bg-white shadow-lg`}>
          <figcaption className="px-1 text-xs text-slate-700">点击局部放大</figcaption>
          <EvidenceImage
            recordingId={recordingId}
            sequence={sequence}
            kind={kind === 'target' ? 'target_crop' : 'crop'}
          />
        </figure>
      ) : null}
    </div>
  );
}

/** 完整图像与点击局部证据共享事件身份，坐标使用物理屏幕单位。 */
export function ScreenshotPreview({ recordingId, sequence, screenshot, target = false }: Readonly<{
  recordingId: string; sequence: number; screenshot: ScreenshotEvidence; target?: boolean;
}>) {
  return (
    <section className="space-y-2">
      <h4 className="text-xs font-semibold">{target ? '点击目标' : '操作结果'}</h4>
      <p className="text-xs text-slate-500">
        {target ? '点击时采样' : '操作后采样'} · {screenshot.captured_at_ms} ms
        {target ? '' : ` · ${screenshot.stabilized ? '画面已稳定' : '等待结束，画面尚未确认稳定'}`}
      </p>
      {screenshot.crop_failure ? <p className="text-xs text-amber-800">局部截图保存失败，完整窗口截图已保留。</p> : null}
      <EvidenceImage
        key={`${sequence}-window`}
        recordingId={recordingId}
        sequence={sequence}
        kind={target ? 'target' : 'window'}
        screenshot={screenshot}
      />
      {screenshot.pointer ? <p className="text-xs text-slate-500">鼠标屏幕坐标：({screenshot.pointer.x}, {screenshot.pointer.y})</p> : null}
    </section>
  );
}
