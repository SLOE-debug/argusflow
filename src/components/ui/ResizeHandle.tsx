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
        "group relative z-20 shrink-0 bg-line outline-none before:absolute " +
        (axis === "x"
          ? "w-px cursor-col-resize before:inset-y-0 before:-inset-x-1"
          : "h-px cursor-row-resize before:inset-x-0 before:-inset-y-1")
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
      onPointerCancel={() => {
        start.current = null;
      }}
      onLostPointerCapture={() => {
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
    >
      <span
        aria-hidden="true"
        className={
          "pointer-events-none absolute rounded-full bg-muted/40 opacity-0 transition-opacity group-hover:opacity-100 group-active:bg-accent/60 group-active:opacity-100 group-focus-visible:bg-accent group-focus-visible:opacity-100 " +
          (axis === "x"
            ? "left-1/2 top-1/2 h-7 w-[3px] -translate-x-1/2 -translate-y-1/2"
            : "left-1/2 top-1/2 h-[3px] w-7 -translate-x-1/2 -translate-y-1/2")
        }
      />
    </div>
  );
}
