import { useLayoutEffect, useState, type RefObject } from "react";
import { selectPlacement } from "./model";

/** 菜单挂在最近的 dialog 内，避免落入模态对话框的 inert 区域。 */
export function useSelectPanel(
  open: boolean,
  trigger: RefObject<HTMLButtonElement | null>,
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
    const position = () =>
      setLayout(
        selectPlacement(
          button.getBoundingClientRect(),
          { width: window.innerWidth, height: window.innerHeight },
          // 包含上下 padding（8px）与 border（2px），避免三项菜单也溢出。
          Math.min(264, Math.max(40, count * 28 + 10)),
        ),
      );
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
  }, [open, trigger, panel, count, close]);
  return { layout, host };
}
