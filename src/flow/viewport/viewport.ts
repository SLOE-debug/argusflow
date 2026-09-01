import type {
  FlowNode,
  FlowRect,
  ViewportTransform,
} from '../types';

/** 画布允许的最大放大倍率。 */
export const MAX_CANVAS_ZOOM = 2.5;

/** 计算一组节点在世界坐标中的最小轴对齐包围盒。 */
export function getNodesBounds(
  nodes: ReadonlyArray<Pick<FlowNode, 'position' | 'size'>>,
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

/** 自动跟随内容时各方向保留的屏幕像素，避免节点被浮动工具遮挡。 */
export type ViewportSafePadding = Readonly<{
  top: number;
  right: number;
  bottom: number;
  left: number;
}>;

/**
 * 仅在目标越出安全可视区时平移视口，并保留用户当前缩放。
 *
 * 四个方向分别计算最小位移；目标比安全区更大时改为沿该轴居中，避免左右或上下
 * 约束互相覆盖后产生方向偏置。
 */
export function ensureBoundsVisibleInViewport(
  bounds: FlowRect,
  viewportSize: Readonly<{ width: number; height: number }>,
  viewport: ViewportTransform,
  padding: ViewportSafePadding,
): ViewportTransform {
  const safeWidth = Math.max(1, viewportSize.width - padding.left - padding.right);
  const safeHeight = Math.max(1, viewportSize.height - padding.top - padding.bottom);
  const screenLeft = bounds.x * viewport.zoom + viewport.x;
  const screenTop = bounds.y * viewport.zoom + viewport.y;
  const screenWidth = bounds.width * viewport.zoom;
  const screenHeight = bounds.height * viewport.zoom;
  const screenRight = screenLeft + screenWidth;
  const screenBottom = screenTop + screenHeight;
  let x = viewport.x;
  let y = viewport.y;

  if (screenWidth > safeWidth) {
    x += padding.left + safeWidth / 2 - (screenLeft + screenWidth / 2);
  } else if (screenLeft < padding.left) {
    x += padding.left - screenLeft;
  } else if (screenRight > viewportSize.width - padding.right) {
    x -= screenRight - (viewportSize.width - padding.right);
  }

  if (screenHeight > safeHeight) {
    y += padding.top + safeHeight / 2 - (screenTop + screenHeight / 2);
  } else if (screenTop < padding.top) {
    y += padding.top - screenTop;
  } else if (screenBottom > viewportSize.height - padding.bottom) {
    y -= screenBottom - (viewportSize.height - padding.bottom);
  }

  return x === viewport.x && y === viewport.y
    ? viewport
    : { x, y, zoom: viewport.zoom };
}

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
