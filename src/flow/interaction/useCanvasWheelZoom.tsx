import {
  useCallback,
  useEffect,
  useRef,
  type WheelEvent as ReactWheelEvent,
} from 'react';

import { screenToWorld, zoomAt } from '../geometry/geometry';
import { useFlowStoreApi } from '../store/store';
import type { FlowPoint } from '../types';

type CanvasWheelZoomOptions = Readonly<{
  maxZoom: number;
  /** 达到业务语义阈值时，可用指针所在结构替换当前画布文档。 */
  onSemanticZoomIn?: (worldPoint: FlowPoint, nextZoom: number) => boolean;
  /** 缩小越过业务语义阈值时，可返回当前结构的父作用域。 */
  onSemanticZoomOut?: (nextZoom: number) => boolean;
}>;

/** 合并同帧滚轮增量，并在普通缩放与业务作用域切换之间建立明确边界。 */
export function useCanvasWheelZoom({
  maxZoom,
  onSemanticZoomIn,
  onSemanticZoomOut,
}: CanvasWheelZoomOptions) {
  const store = useFlowStoreApi();
  /** 同一动画帧内累计的滚轮垂直增量。 */
  const wheelDelta = useRef(0);
  /** 最后一次滚轮事件相对画布左上角的坐标。 */
  const wheelPoint = useRef<FlowPoint | null>(null);
  /** 等待应用滚轮缩放的动画帧 ID。 */
  const wheelFrame = useRef<number | null>(null);

  const handleWheel = useCallback((event: ReactWheelEvent<HTMLDivElement>) => {
    event.preventDefault();
    const bounds = event.currentTarget.getBoundingClientRect();
    wheelDelta.current += event.deltaY;
    wheelPoint.current = {
      x: event.clientX - bounds.left,
      y: event.clientY - bounds.top,
    };
    wheelFrame.current ??= requestAnimationFrame(() => {
      const viewport = store.getState().viewport;
      const screenPoint = wheelPoint.current;
      /** 当前帧累计的滚轮增量会在读取后立即清零。 */
      const deltaY = wheelDelta.current;
      wheelFrame.current = null;
      wheelDelta.current = 0;
      wheelPoint.current = null;
      if (!screenPoint) return;

      /** 只限制放大上限；极小正数防止连续缩小导致浮点倍率归零。 */
      const nextZoom = Math.min(
        maxZoom,
        Math.max(Number.MIN_VALUE, viewport.zoom * Math.exp(-deltaY * 0.0015)),
      );
      const worldPoint = screenToWorld(screenPoint, viewport);
      if (onSemanticZoomIn?.(worldPoint, nextZoom)) return;
      if (onSemanticZoomOut?.(nextZoom)) return;
      store.getState().setViewport(zoomAt(viewport, screenPoint, nextZoom));
    });
  }, [maxZoom, onSemanticZoomIn, onSemanticZoomOut, store]);

  useEffect(() => () => {
    if (wheelFrame.current !== null) cancelAnimationFrame(wheelFrame.current);
  }, []);

  return handleWheel;
}
