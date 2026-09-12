import type { ViewportTransform } from "../../../../flow";
import type { ThemeDefinition } from "../../../../features/themes";

const smooth = (from: number, to: number, value: number) => {
  const t = Math.max(0, Math.min(1, (value - from) / (to - from)));
  return t * t * (3 - 2 * t);
};

/** 点阵固定在场景坐标中；层级样式仅取决于屏幕间距，换档时连续交接。 */
export function drawBackground(
  context: CanvasRenderingContext2D,
  camera: ViewportTransform,
  width: number,
  height: number,
  colors: ThemeDefinition["colors"],
): void {
  context.save();
  const paper = context.createLinearGradient(0, 0, width * 0.35, height);
  paper.addColorStop(0, colors.surface);
  paper.addColorStop(0.65, colors.canvas);
  paper.addColorStop(1, colors.canvas);
  context.fillStyle = paper;
  context.fillRect(0, 0, width, height);

  // 三档四倍间距：细点逐渐浮现，主点保留方位感，远档逐渐隐去。
  // 点数只与屏幕尺寸有关，不随无限平移或深层缩放增长。
  const level = Math.floor(Math.log(camera.zoom) / Math.log(4));
  const spacing = 24 * (camera.zoom / 4 ** level);
  context.fillStyle = colors.grid;
  for (const step of [spacing / 4, spacing, spacing * 4]) {
    const emphasis = smooth(24, 96, step);
    const opacity =
      step <= 24
        ? 0.64 * smooth(8, 24, step)
        : (0.64 - 0.22 * emphasis) * (1 - smooth(96, 384, step));
    if (opacity < 0.01) continue;
    const radius = 0.9 + 0.4 * emphasis;
    const startX = (((camera.x % step) + step) % step) - step;
    const startY = (((camera.y % step) + step) % step) - step;
    context.globalAlpha = opacity;
    context.beginPath();
    for (let x = startX; x <= width + radius; x += step)
      for (let y = startY; y <= height + radius; y += step) {
        context.moveTo(x + radius, y);
        context.arc(x, y, radius, 0, Math.PI * 2);
      }
    context.fill();
  }
  context.restore();
}
