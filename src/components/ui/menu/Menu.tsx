import { useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { MenuPanel } from "./MenuPanel";
import type { MenuItem } from "./model";

/** 紧凑级联菜单的生命周期入口，所有层共享外部点击关闭边界。 */
export function Menu({
  x,
  y,
  label,
  items,
  onClose,
}: {
  readonly x: number;
  readonly y: number;
  readonly label: string;
  readonly items: readonly MenuItem[];
  readonly onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const close = (event: PointerEvent) => {
      if (event.target instanceof Node && !ref.current?.contains(event.target))
        onClose();
    };
    window.addEventListener("pointerdown", close);
    window.addEventListener("resize", onClose);
    return () => {
      window.removeEventListener("pointerdown", close);
      window.removeEventListener("resize", onClose);
    };
  }, [onClose]);
  return createPortal(
    <div
      ref={ref}
      onPointerDown={(event) => event.stopPropagation()}
      onContextMenu={(event) => event.preventDefault()}
    >
      <MenuPanel
        key={x + ":" + y}
        items={items}
        label={label}
        anchor={{ type: "point", x, y }}
        focusOnOpen
        onBack={onClose}
        onClose={onClose}
      />
    </div>,
    document.body,
  );
}
