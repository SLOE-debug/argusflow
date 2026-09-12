import {
  worldToScreen,
  type SnapGuide,
  type ViewportTransform,
} from "../../../../flow";

/** 在 CSS 像素空间绘制紫色辅助线，缩放不改变线宽、虚线间隔或端点标记。 */
export function drawSnapGuides(
  context: CanvasRenderingContext2D,
  guides: readonly SnapGuide[],
  view: ViewportTransform,
  color: string,
): void {
  if (!guides.length) return;
  context.save();
  context.strokeStyle = color;
  context.lineWidth = 1;
  context.globalAlpha = 0.95;
  for (const guide of guides) {
    const vertical = guide.axis === "x";
    const from = worldToScreen(
      {
        x: vertical ? guide.position : guide.start,
        y: vertical ? guide.start : guide.position,
      },
      view,
    );
    const to = worldToScreen(
      {
        x: vertical ? guide.position : guide.end,
        y: vertical ? guide.end : guide.position,
      },
      view,
    );
    /** 奇数像素线宽沿半像素落点绘制，短端点标记表示参考线的覆盖范围。 */
    const position = Math.round(vertical ? from.x : from.y) + 0.5;
    const start = Math.round(vertical ? from.y : from.x) - 8 + 0.5;
    const end = Math.round(vertical ? to.y : to.x) + 8 + 0.5;
    context.setLineDash([4, 3]);
    context.beginPath();
    context.moveTo(vertical ? position : start, vertical ? start : position);
    context.lineTo(vertical ? position : end, vertical ? end : position);
    context.stroke();
    context.setLineDash([]);
    context.beginPath();
    for (const offset of [start, end]) {
      context.moveTo(
        vertical ? position - 3 : offset,
        vertical ? offset : position - 3,
      );
      context.lineTo(
        vertical ? position + 3 : offset,
        vertical ? offset : position + 3,
      );
    }
    context.stroke();
  }
  context.restore();
}
