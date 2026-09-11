import { useLayoutEffect, useState, type RefObject } from "react";
import type { MenuAnchor } from "./model";

/** 按浮层实际尺寸避让窗口边缘；级联菜单优先向右，空间不足时向左。 */
export function useMenuPosition(
  ref: RefObject<HTMLDivElement | null>,
  anchor: MenuAnchor,
) {
  const [position, setPosition] = useState({ left: 0, top: 0 });
  useLayoutEffect(() => {
    const panel = ref.current;
    if (!panel) return;
    const bounds = panel.getBoundingClientRect();
    const trigger =
      anchor.type === "submenu"
        ? anchor.trigger.getBoundingClientRect()
        : undefined;
    const x = anchor.type === "point" ? anchor.x : trigger!.right + 4;
    const y = anchor.type === "point" ? anchor.y : trigger!.top - 4;
    const left = Math.max(
      8,
      Math.min(
        trigger && x + bounds.width > window.innerWidth - 8
          ? trigger.left - bounds.width - 4
          : x,
        window.innerWidth - bounds.width - 8,
      ),
    );
    const top = Math.max(
      8,
      Math.min(y, window.innerHeight - bounds.height - 8),
    );
    setPosition((current) =>
      current.left === left && current.top === top ? current : { left, top },
    );
  }, [anchor, ref]);
  return position;
}
