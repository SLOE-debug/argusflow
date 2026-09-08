import type { ScreenshotEvidence } from '../../features/recorder';

/** 点击框与图像共享 viewBox，负屏幕原点和缩放不影响定位。 */
export function ClickEvidenceMarker({ screenshot }: Readonly<{ screenshot: ScreenshotEvidence }>) {
  if (!screenshot.pointer || !screenshot.click_color) return null;
  const x = screenshot.pointer.x - screenshot.screen_bounds.x;
  const y = screenshot.pointer.y - screenshot.screen_bounds.y;
  if (x < 0 || y < 0 || x >= screenshot.width || y >= screenshot.height) return null;
  const left = Math.max(0, x - 16);
  const top = Math.max(0, y - 16);
  /** 32px 定位框在图像边缘按实际交集裁切。 */
  const bounds = { x: left, y: top, width: Math.min(screenshot.width, x + 16) - left,
    height: Math.min(screenshot.height, y + 16) - top };
  return (
    <svg
      aria-label="点击位置框"
      role="img"
      viewBox={`0 0 ${screenshot.width} ${screenshot.height}`}
      className="pointer-events-none absolute inset-0 size-full"
    >
      <rect
        {...bounds}
        fill="none"
        stroke={`rgb(${screenshot.click_color.join(',')})`}
        strokeWidth={3}
        vectorEffect="non-scaling-stroke"
      />
    </svg>
  );
}
