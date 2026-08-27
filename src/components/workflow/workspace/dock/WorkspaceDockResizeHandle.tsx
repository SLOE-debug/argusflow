import {
  useRef,
  type KeyboardEvent as ReactKeyboardEvent,
  type PointerEvent as ReactPointerEvent,
} from 'react';

type WorkspaceDockResizeHandleProps = Readonly<{
  /** 当前 Dock 高度。 */
  height: number;
  /** 受当前 Workspace 高度约束的最小值。 */
  minHeight: number;
  /** 受当前 Workspace 高度约束的最大值。 */
  maxHeight: number;
  /** 双击恢复的偏好高度。 */
  defaultHeight: number;
  /** 发布经过边界限制的新高度。 */
  onHeightChange: (height: number) => void;
}>;

type ResizeOrigin = Readonly<{
  /** 拖拽开始时的屏幕纵坐标。 */
  pointerY: number;
  /** 拖拽开始时的 Dock 高度。 */
  height: number;
}>;

/** 把 Dock 高度约束在当前可用中央区域内。 */
export function clampDockHeight(
  height: number,
  minHeight: number,
  maxHeight: number,
): number {
  return Math.min(maxHeight, Math.max(minHeight, height));
}

/** 支持指针、键盘与双击复位的水平 Workspace 分隔条。 */
export function WorkspaceDockResizeHandle({
  height,
  minHeight,
  maxHeight,
  defaultHeight,
  onHeightChange,
}: WorkspaceDockResizeHandleProps) {
  /** 当前拖拽的初始坐标；不存在时表示没有活动手势。 */
  const resizeOrigin = useRef<ResizeOrigin | null>(null);

  const handlePointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return;
    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    resizeOrigin.current = { pointerY: event.clientY, height };
  };

  const handlePointerMove = (event: ReactPointerEvent<HTMLDivElement>) => {
    const origin = resizeOrigin.current;
    if (!origin || !event.currentTarget.hasPointerCapture(event.pointerId)) return;
    onHeightChange(clampDockHeight(
      origin.height + origin.pointerY - event.clientY,
      minHeight,
      maxHeight,
    ));
  };

  const clearPointer = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    resizeOrigin.current = null;
  };

  const handleKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    const step = event.shiftKey ? 40 : 10;
    if (event.key === 'ArrowUp' || event.key === 'ArrowDown') {
      event.preventDefault();
      onHeightChange(clampDockHeight(
        height + (event.key === 'ArrowUp' ? step : -step),
        minHeight,
        maxHeight,
      ));
    }
  };

  return (
    <div
      role="separator"
      aria-label="调整底部面板高度"
      aria-orientation="horizontal"
      aria-valuemin={Math.round(minHeight)}
      aria-valuemax={Math.round(maxHeight)}
      aria-valuenow={Math.round(height)}
      tabIndex={0}
      className="group absolute inset-x-0 top-0 z-20 h-2 -translate-y-1/2 cursor-row-resize touch-none outline-none"
      onDoubleClick={() => onHeightChange(clampDockHeight(
        defaultHeight,
        minHeight,
        maxHeight,
      ))}
      onKeyDown={handleKeyDown}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={clearPointer}
      onPointerCancel={clearPointer}
    >
      <span className="absolute inset-x-0 top-1/2 h-px -translate-y-1/2 bg-slate-200 transition-colors group-hover:bg-blue-400 group-focus:bg-blue-500" />
    </div>
  );
}
