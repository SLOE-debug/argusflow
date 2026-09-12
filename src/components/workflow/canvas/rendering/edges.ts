import {
  routeEdge,
  type RoutedPath,
  type RoutingObstacles,
  type FlowAnchor,
} from "../../../../flow";
import type { WirePreview } from "../scene";

/** 帧间复用临时路线；每次指针更新只保留最新草稿。 */
const routes = new WeakMap<WirePreview, RoutedPath>();
export function wireRoute(
  wire: WirePreview,
  obstacles: RoutingObstacles,
): RoutedPath {
  let route = routes.get(wire);
  if (!route) {
    route = routeEdge(wire.source, wire.target, obstacles);
    routes.set(wire, route);
  }
  return route;
}
/** 实际圆弧和箭头使用与命中完全相同的路径段。 */
export function drawEdge(
  context: CanvasRenderingContext2D,
  path: RoutedPath,
  color: string,
  zoom: number,
  highlighted = false,
  selected = false,
): void {
  if (!path.segments.length) return;
  // 缩小时随节点等比缩小，放大时保持适中的屏幕尺寸。
  const unit = 1 / Math.max(1, zoom);
  context.save();
  context.beginPath();
  context.moveTo(path.segments[0].from.x, path.segments[0].from.y);
  for (const segment of path.segments) {
    if (segment.kind === "line") context.lineTo(segment.to.x, segment.to.y);
    else
      context.arc(
        segment.center.x,
        segment.center.y,
        segment.radius,
        segment.start,
        segment.start + segment.sweep,
        segment.sweep < 0,
      );
  }
  context.strokeStyle = color;
  context.lineJoin = "round";
  context.lineCap = "round";
  if (path.status === "blocked") context.setLineDash([5 * unit, 4 * unit]);
  if (highlighted || selected) {
    context.globalAlpha = selected ? 0.16 : 0.1;
    context.lineWidth = (selected ? 9 : 7) * unit;
    context.stroke();
  }
  context.globalAlpha = highlighted || selected ? 1 : 0.85;
  context.lineWidth = (selected ? 2.5 : highlighted ? 2.2 : 1.7) * unit;
  context.stroke();
  context.setLineDash([]);
  const last = path.segments.at(-1)!;
  const angle =
    last.kind === "arc"
      ? last.start + last.sweep + (Math.sign(last.sweep) * Math.PI) / 2
      : Math.atan2(last.to.y - last.from.y, last.to.x - last.from.x);
  const size = 6 * unit,
    end = last.to;
  context.beginPath();
  context.moveTo(
    end.x - size * Math.cos(angle - 0.5),
    end.y - size * Math.sin(angle - 0.5),
  );
  context.lineTo(end.x, end.y);
  context.lineTo(
    end.x - size * Math.cos(angle + 0.5),
    end.y - size * Math.sin(angle + 0.5),
  );
  context.stroke();
  context.restore();
}
/** 端点操作点保持屏幕像素尺寸，覆盖在卡片和线路之上。 */
export function drawEdgeHandle(
  context: CanvasRenderingContext2D,
  anchor: FlowAnchor,
  color: string,
  background: string,
  zoom: number,
): void {
  if (zoom < 0.25) return;
  context.save();
  context.beginPath();
  context.arc(anchor.point.x, anchor.point.y, 4.5 / zoom, 0, Math.PI * 2);
  context.fillStyle = background;
  context.fill();
  context.strokeStyle = color;
  context.lineWidth = 1.8 / zoom;
  context.stroke();
  context.restore();
}
