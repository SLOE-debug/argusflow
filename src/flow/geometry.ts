import type { FlowAnchorSide, FlowPoint, FlowRect, ViewportTransform } from './types';

/** 将屏幕坐标投影到画布逻辑坐标。 */
export function screenToWorld(point: FlowPoint, viewport: ViewportTransform): FlowPoint {
  return { x: (point.x - viewport.x) / viewport.zoom, y: (point.y - viewport.y) / viewport.zoom };
}

/** 将画布逻辑坐标投影到屏幕坐标。 */
export function worldToScreen(point: FlowPoint, viewport: ViewportTransform): FlowPoint {
  return { x: point.x * viewport.zoom + viewport.x, y: point.y * viewport.zoom + viewport.y };
}

/** 以鼠标位置为定点缩放视口。 */
export function zoomAt(viewport: ViewportTransform, screenPoint: FlowPoint, nextZoom: number): ViewportTransform {
  const world = screenToWorld(screenPoint, viewport);
  return {
    zoom: nextZoom,
    x: screenPoint.x - world.x * nextZoom,
    y: screenPoint.y - world.y * nextZoom,
  };
}

/** 返回矩形对应边的中点。 */
export function anchorPoint(rect: FlowRect, side: FlowAnchorSide, gap = 0): FlowPoint {
  switch (side) {
    case 'top': return { x: rect.x + rect.width / 2, y: rect.y - gap };
    case 'right': return { x: rect.x + rect.width + gap, y: rect.y + rect.height / 2 };
    case 'bottom': return { x: rect.x + rect.width / 2, y: rect.y + rect.height + gap };
    case 'left': return { x: rect.x - gap, y: rect.y + rect.height / 2 };
  }
}

/** 判断两个轴对齐矩形是否重叠。 */
export function rectsIntersect(a: FlowRect, b: FlowRect): boolean {
  return a.x <= b.x + b.width && a.x + a.width >= b.x && a.y <= b.y + b.height && a.y + a.height >= b.y;
}

/** 将两个点规范化为正尺寸矩形。 */
export function rectFromPoints(a: FlowPoint, b: FlowPoint): FlowRect {
  return { x: Math.min(a.x, b.x), y: Math.min(a.y, b.y), width: Math.abs(a.x - b.x), height: Math.abs(a.y - b.y) };
}

/** 判断屏幕视口扩展指定逻辑边距后是否包含矩形。 */
export function isRectVisible(rect: FlowRect, viewport: ViewportTransform, width: number, height: number, margin = 180): boolean {
  const visible: FlowRect = {
    x: (-viewport.x - margin) / viewport.zoom,
    y: (-viewport.y - margin) / viewport.zoom,
    width: (width + margin * 2) / viewport.zoom,
    height: (height + margin * 2) / viewport.zoom,
  };
  return rectsIntersect(rect, visible);
}

/** 计算一组点的包围盒。 */
export function pointsBounds(points: FlowPoint[]): FlowRect {
  const xs = points.map((point) => point.x);
  const ys = points.map((point) => point.y);
  const minX = Math.min(...xs);
  const minY = Math.min(...ys);
  return { x: minX, y: minY, width: Math.max(...xs) - minX, height: Math.max(...ys) - minY };
}
