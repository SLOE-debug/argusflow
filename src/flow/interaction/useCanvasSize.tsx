import { useEffect, useState, type RefObject } from 'react';

/** 画布容器的屏幕像素尺寸。 */
export type CanvasSize = Readonly<{
  width: number;
  height: number;
}>;

const INITIAL_CANVAS_SIZE: CanvasSize = { width: 1, height: 1 };

/** 持续观察画布容器尺寸，为视口裁剪和 SVG 边界提供稳定输入。 */
export function useCanvasSize(
  containerRef: RefObject<HTMLDivElement | null>,
): CanvasSize {
  const [size, setSize] = useState<CanvasSize>(INITIAL_CANVAS_SIZE);

  useEffect(() => {
    const element = containerRef.current;
    if (!element) return undefined;

    const observer = new ResizeObserver(([entry]) => {
      if (!entry) return;
      /** WebView 可能重复报告同一尺寸；相等时保留旧对象，避免画布空转。 */
      const nextSize: CanvasSize = {
        width: Math.round(entry.contentRect.width),
        height: Math.round(entry.contentRect.height),
      };
      setSize((currentSize) => (
        currentSize.width === nextSize.width && currentSize.height === nextSize.height
          ? currentSize
          : nextSize
      ));
    });

    observer.observe(element);
    return () => observer.disconnect();
  }, [containerRef]);

  return size;
}
