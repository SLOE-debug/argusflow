import { useEffect, useState, type RefObject } from "react";
export function useElementSize(ref: RefObject<HTMLElement | null>): {
  readonly width: number;
  readonly height: number;
} {
  const [size, setSize] = useState({ width: 800, height: 600 });
  useEffect(() => {
    const element = ref.current;
    if (!element) return;
    const update = () =>
      setSize({ width: element.clientWidth, height: element.clientHeight });
    update();
    const observer = new ResizeObserver(update);
    observer.observe(element);
    return () => observer.disconnect();
  }, [ref]);
  return size;
}
