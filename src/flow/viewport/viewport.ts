import type {
  FlowNode,
  FlowRect,
  ViewportTransform,
} from '../types';

/** 画布允许的最大放大倍率。 */
export const MAX_CANVAS_ZOOM = 2.5;

/** 计算一组节点在世界坐标中的最小轴对齐包围盒。 */
export function getNodesBounds(
  nodes: ReadonlyArray<FlowNode>,
): FlowRect | null {
  if (nodes.length === 0) return null;

  let minX = Number.POSITIVE_INFINITY;
  let minY = Number.POSITIVE_INFINITY;
  let maxX = Number.NEGATIVE_INFINITY;
  let maxY = Number.NEGATIVE_INFINITY;

  for (const node of nodes) {
    minX = Math.min(minX, node.position.x);
    minY = Math.min(minY, node.position.y);
    maxX = Math.max(maxX, node.position.x + node.size.width);
    maxY = Math.max(maxY, node.position.y + node.size.height);
  }

  return {
    x: minX,
    y: minY,
    width: maxX - minX,
    height: maxY - minY,
  };
}

/** 在不改变缩放倍率的前提下，将世界坐标包围盒居中到视口。 */
export function centerBoundsInViewport(
  bounds: FlowRect,
  viewportSize: Readonly<{ width: number; height: number }>,
  zoom: number,
): ViewportTransform {
  return {
    x: viewportSize.width / 2 - (bounds.x + bounds.width / 2) * zoom,
    y: viewportSize.height / 2 - (bounds.y + bounds.height / 2) * zoom,
    zoom,
  };
}

type FitBoundsOptions = Readonly<{
  /** 内容与视口边缘之间保留的屏幕像素。 */
  padding?: number;
  /** 自动适应允许的最小缩放倍率。 */
  minZoom?: number;
  /** 自动适应允许的最大缩放倍率。 */
  maxZoom?: number;
}>;

/** 计算完整容纳指定世界坐标包围盒的居中视口。 */
export function fitBoundsToViewport(
  bounds: FlowRect,
  viewportSize: Readonly<{ width: number; height: number }>,
  options?: FitBoundsOptions,
): ViewportTransform {
  const padding = Math.max(0, options?.padding ?? 64);
  const minZoom = options?.minZoom ?? 0.15;
  const maxZoom = options?.maxZoom ?? MAX_CANVAS_ZOOM;
  const availableWidth = Math.max(1, viewportSize.width - padding * 2);
  const availableHeight = Math.max(1, viewportSize.height - padding * 2);
  const contentWidth = Math.max(1, bounds.width);
  const contentHeight = Math.max(1, bounds.height);
  const fittedZoom = Math.min(
    availableWidth / contentWidth,
    availableHeight / contentHeight,
  );
  const zoom = Math.min(maxZoom, Math.max(minZoom, fittedZoom));

  return centerBoundsInViewport(bounds, viewportSize, zoom);
}
