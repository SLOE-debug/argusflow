import { useLayoutEffect, useRef, useState, type PointerEvent, type ReactNode } from 'react';
import { useEvidenceImage, type PrivacyRect, type PrivacyImageKind, type ScreenshotEvidence } from '../../features/recorder';
import { Button } from '../ui';

/** 在原图坐标框选隐私区域，缩放展示不会影响后端保存范围。 */
export function PrivacyScreenshotEditor({ recordingId, sequence, kind, screenshot, highlights, disabled, onMosaic, overlay, editing = true }: Readonly<{
  recordingId: string;
  sequence: number;
  kind: PrivacyImageKind;
  screenshot: ScreenshotEvidence;
  highlights: readonly PrivacyRect[];
  disabled: boolean;
  onMosaic: (rect: PrivacyRect) => void;
  /** 与图像共享坐标容器的播放提示。 */
  overlay?: ReactNode;
  /** 只有框选模式才截获拖动。 */
  editing?: boolean;
}>) {
  const image = useEvidenceImage(recordingId, sequence, kind);
  const [rect, setRect] = useState<PrivacyRect | null>(null);
  /** 使用画面区真实剩余尺寸等比适配，框选层与图片始终重合。 */
  const viewport = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  useLayoutEffect(() => {
    const element = viewport.current;
    if (!element) return;
    const update = () => {
      const scale = Math.min(element.clientWidth / screenshot.width, element.clientHeight / screenshot.height);
      setSize({ width: screenshot.width * scale, height: screenshot.height * scale });
    };
    const observer = new ResizeObserver(update);
    observer.observe(element);
    update();
    return () => observer.disconnect();
  }, [screenshot.width, screenshot.height]);
  /** 按下点保留原图坐标，不受 React 更新时机影响。 */
  const origin = useRef<Readonly<{ x: number; y: number }> | null>(null);
  const point = (event: PointerEvent<HTMLDivElement>) => {
    const bounds = event.currentTarget.getBoundingClientRect();
    return {
      x: Math.max(0, Math.min(screenshot.width, Math.round((event.clientX - bounds.left) / bounds.width * screenshot.width))),
      y: Math.max(0, Math.min(screenshot.height, Math.round((event.clientY - bounds.top) / bounds.height * screenshot.height))),
    };
  };
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2">
      {editing ? <p className="text-xs leading-5 text-slate-600">在画面上拖出一个框，再点击“遮盖框选区域”。只处理当前这张截图。</p> : null}
      <div ref={viewport} className="flex min-h-0 flex-1 items-center justify-center overflow-hidden">
      {image.type === 'ready' ? (
        // 截图框选属于画布交互，使用原图坐标处理 pointer capture。
        <div
          className="relative mx-auto w-fit max-w-full touch-none select-none overflow-hidden rounded-lg border border-slate-200 bg-slate-100"
          style={size}
          onPointerDown={(event) => {
            if (disabled || !editing || event.button !== 0) return;
            event.currentTarget.setPointerCapture(event.pointerId);
            origin.current = point(event);
            setRect(null);
          }}
          onPointerMove={(event) => {
            if (!origin.current) return;
            const end = point(event);
            setRect({ x: Math.min(origin.current.x, end.x), y: Math.min(origin.current.y, end.y), width: Math.abs(end.x - origin.current.x), height: Math.abs(end.y - origin.current.y) });
          }}
          onPointerUp={() => { origin.current = null; }}
          onPointerCancel={() => { origin.current = null; setRect(null); }}
        >
          <img
            src={image.url}
            alt="待检查的录制截图"
            draggable={false}
            className={`block size-full ${editing ? 'cursor-crosshair' : ''}`}
          />
          <svg
            aria-hidden="true"
            className="pointer-events-none absolute inset-0 size-full"
            viewBox={`0 0 ${screenshot.width} ${screenshot.height}`}
          >
            {[...highlights, ...(rect ? [rect] : [])].map((area, index) => (
              <rect
                key={index}
                {...area}
                className="fill-blue-500/25 stroke-blue-600"
                strokeWidth={2}
                vectorEffect="non-scaling-stroke"
              />
            ))}
          </svg>
          {overlay}
        </div>
      ) : <p role="status">{image.type === 'loading' ? '正在读取截图…' : '截图读取失败，请重新打开录制后重试。'}</p>}
      </div>
      {editing ? <Button
        disabled={disabled || !rect?.width || !rect.height}
        onClick={() => { if (rect) onMosaic(rect); }}
      >遮盖框选区域</Button> : null}
    </div>
  );
}
