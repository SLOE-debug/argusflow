import { useLayoutEffect, useState, type RefObject } from "react";
import { selectPlacement } from "./model";

/** 菜单挂在最近的 dialog 内，避免落入模态对话框的 inert 区域。 */
export function useSelectPanel(
  open: boolean,
  trigger: RefObject<HTMLElement | null>,
  panel: RefObject<HTMLDivElement | null>,
  count: number,
  close: () => void,
) {
  const [layout, setLayout] = useState<ReturnType<typeof selectPlacement>>();
  const [host, setHost] = useState<HTMLElement>();
  useLayoutEffect(() => {
    const button = trigger.current;
    if (!open || !button) return;
    setHost(button.closest("dialog") ?? document.body);
    const position = () => {
      const rect = button.getBoundingClientRect();
      setLayout(
        selectPlacement(
          {
            left: rect.left,
            top: rect.top,
            bottom: rect.bottom,
            width: Math.max(
              rect.width,
              panel.current?.getBoundingClientRect().width ?? 0,
            ),
          },
          { width: window.innerWidth, height: window.innerHeight },
          // 使用换行后的实际内容高度；首次挂载前才按行数估算。
          Math.min(
            320,
            Math.max(
              40,
              panel.current ? panel.current.scrollHeight + 2 : count * 28 + 10,
            ),
          ),
        ),
      );
    };
    const outside = (event: Event) => {
      if (
        event.target instanceof Node &&
        !button.contains(event.target) &&
        !panel.current?.contains(event.target)
      )
        close();
    };
    position();
    const observer = new ResizeObserver(position);
    observer.observe(button);
    if (panel.current) observer.observe(panel.current);
    window.addEventListener("resize", position);
    window.addEventListener("scroll", position, true);
    document.addEventListener("pointerdown", outside);
    document.addEventListener("focusin", outside);
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", position);
      window.removeEventListener("scroll", position, true);
      document.removeEventListener("pointerdown", outside);
      document.removeEventListener("focusin", outside);
    };
  }, [open, trigger, panel, count, close, host]);
  return { layout, host };
}
