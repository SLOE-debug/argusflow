import { useEffect, useState, type RefObject } from 'react';

/** 按可用宽度选择 1/2/5 刻度，短录制和窄面板也不重叠标签。 */
export function useTimelineScale(track: RefObject<HTMLDivElement | null>, maximum: number, step: number) {
  const [width, setWidth] = useState(800);
  useEffect(() => {
    const element = track.current;
    if (!element) return;
    const observer = new ResizeObserver(([entry]) => {
      if (entry.contentRect.width > 0) setWidth(entry.contentRect.width);
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, [track]);
  /** 每个主刻度至少预留 72px，时间标签保留可读间距。 */
  const rough = Math.max(step, maximum / Math.max(1, Math.floor(width / 72)));
  const magnitude = 10 ** Math.floor(Math.log10(rough));
  const interval = Math.max(step, ([1, 2, 5, 10].find((multiple) => multiple * magnitude >= rough) ?? 10) * magnitude);
  const subdivisions = interval / 10 >= step ? 10 : interval / 5 >= step ? 5 : 1;
  const ticks = Array.from({ length: Math.floor(maximum / interval * subdivisions) + 1 }, (_, index) => index * interval / subdivisions);
  return { subdivisions, ticks };
}
