import { useRef } from "react";
/** 面板尺寸同时支持拖拽和方向键。 */
export function ResizeHandle({
  axis,
  value,
  min,
  max,
  onChange,
  reverse = false,
}: {
  readonly axis: "x" | "y";
  readonly value: number;
  readonly min: number;
  readonly max: number;
  readonly onChange: (value: number) => void;
  readonly reverse?: boolean;
}) {
  const start = useRef<{ point: number; value: number } | null>(null);
  const update = (next: number) => onChange(Math.max(min, Math.min(max, next)));
  return (
    <div
      role="separator"
      tabIndex={0}
      aria-label="调整面板尺寸"
      aria-orientation={axis === "x" ? "vertical" : "horizontal"}
      aria-valuenow={Math.round(value)}
      aria-valuemin={min}
      aria-valuemax={max}
      className={
        axis === "x"
          ? "z-20 w-1 shrink-0 cursor-col-resize hover:bg-accent/40 focus:bg-accent/40"
          : "z-20 h-1 shrink-0 cursor-row-resize hover:bg-accent/40 focus:bg-accent/40"
      }
      onPointerDown={(event) => {
        start.current = {
          point: axis === "x" ? event.clientX : event.clientY,
          value,
        };
        event.currentTarget.setPointerCapture(event.pointerId);
      }}
      onPointerMove={(event) => {
        if (start.current)
          update(
            start.current.value +
              ((axis === "x" ? event.clientX : event.clientY) -
                start.current.point) *
                (reverse ? -1 : 1),
          );
      }}
      onPointerUp={() => {
        start.current = null;
      }}
      onKeyDown={(event) => {
        if (
          ["ArrowLeft", "ArrowUp", "ArrowRight", "ArrowDown"].includes(
            event.key,
          )
        ) {
          event.preventDefault();
          update(
            value + (["ArrowLeft", "ArrowUp"].includes(event.key) ? -10 : 10),
          );
        }
      }}
    />
  );
}
