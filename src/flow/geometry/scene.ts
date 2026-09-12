import type { FlowPoint, FlowRect, ViewportTransform } from "../types";
/** 子图相对父图的仿射变换，进入时相机等价重定位。 */
export function compose(
  parent: ViewportTransform,
  child: ViewportTransform,
): ViewportTransform {
  return {
    x: parent.x + child.x * parent.zoom,
    y: parent.y + child.y * parent.zoom,
    zoom: parent.zoom * child.zoom,
  };
}
export function inverse(view: ViewportTransform): ViewportTransform {
  return {
    x: -view.x / view.zoom,
    y: -view.y / view.zoom,
    zoom: 1 / view.zoom,
  };
}
export function transformPoint(
  point: FlowPoint,
  transform: ViewportTransform,
): FlowPoint {
  return {
    x: point.x * transform.zoom + transform.x,
    y: point.y * transform.zoom + transform.y,
  };
}
export function contains(rect: FlowRect, point: FlowPoint): boolean {
  return (
    point.x >= rect.x &&
    point.x <= rect.x + rect.width &&
    point.y >= rect.y &&
    point.y <= rect.y + rect.height
  );
}
export function fitBounds(
  rect: FlowRect,
  width: number,
  height: number,
  padding = 50,
  maxZoom = 1.2,
): ViewportTransform {
  const zoom = Math.min(
    maxZoom,
    Math.max(
      0.000001,
      Math.min(
        (width - padding * 2) / Math.max(1, rect.width),
        (height - padding * 2) / Math.max(1, rect.height),
      ),
    ),
  );
  return {
    zoom,
    x: (width - rect.width * zoom) / 2 - rect.x * zoom,
    y: (height - rect.height * zoom) / 2 - rect.y * zoom,
  };
}
/** 圆角正交线，端点来自同一坐标系。 */
export function edgePath(source: FlowPoint, target: FlowPoint): string {
  if (target.x >= source.x + 32) {
    const mid = (source.x + target.x) / 2;
    return (
      "M " +
      source.x +
      " " +
      source.y +
      " C " +
      mid +
      " " +
      source.y +
      ", " +
      mid +
      " " +
      target.y +
      ", " +
      target.x +
      " " +
      target.y
    );
  }
  const y = Math.max(source.y, target.y) + 56;
  return (
    "M " +
    source.x +
    " " +
    source.y +
    " L " +
    (source.x + 24) +
    " " +
    source.y +
    " L " +
    (source.x + 24) +
    " " +
    y +
    " L " +
    (target.x - 24) +
    " " +
    y +
    " L " +
    (target.x - 24) +
    " " +
    target.y +
    " L " +
    target.x +
    " " +
    target.y
  );
}
