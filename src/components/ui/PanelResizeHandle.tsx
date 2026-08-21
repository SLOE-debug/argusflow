import {
  useRef,
  type KeyboardEvent as ReactKeyboardEvent,
  type PointerEvent as ReactPointerEvent,
} from 'react';

/** 面板位于工作区的哪一侧，用于确定指针位移的宽度增减方向。 */
export type ResizablePanelSide = 'left' | 'right';

export type PanelResizeHandleProps = Readonly<{
  /** 调整宽度的面板所在侧。 */
  side: ResizablePanelSide;
  /** 当前面板宽度。 */
  width: number;
  /** 面板允许的最小宽度。 */
  minWidth: number;
  /** 面板允许的最大宽度。 */
  maxWidth: number;
  /** 恢复宽度时使用的默认值。 */
  defaultWidth: number;
  /** 发布经过边界约束的新宽度。 */
  onWidthChange: (width: number) => void;
}>;

type ResizeOrigin = Readonly<{
  /** 拖拽开始时的屏幕横坐标。 */
  pointerX: number;
  /** 拖拽开始时的面板宽度。 */
  width: number;
}>;

/** 将面板宽度限制在公开布局契约内。 */
function clampWidth(width: number, minWidth: number, maxWidth: number): number {
  return Math.min(maxWidth, Math.max(minWidth, width));
}

/** 左右面板共用的可访问拖拽分隔条，支持指针、方向键与双击复位。 */
export function PanelResizeHandle({
  side,
  width,
  minWidth,
  maxWidth,
  defaultWidth,
  onWidthChange,
}: PanelResizeHandleProps) {
  /** 当前指针拖拽的初始坐标与宽度；不存在时表示未在拖拽。 */
  const resizeOrigin = useRef<ResizeOrigin | null>(null);
  const positionClassName = side === 'left'
    ? 'right-0 translate-x-1/2'
    : 'left-0 -translate-x-1/2';

  const handlePointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return;
    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    resizeOrigin.current = {
      pointerX: event.clientX,
      width,
    };
  };

  const handlePointerMove = (event: ReactPointerEvent<HTMLDivElement>) => {
    const origin = resizeOrigin.current;
    if (!origin) return;

    const pointerDelta = event.clientX - origin.pointerX;
    const widthDelta = side === 'left' ? pointerDelta : -pointerDelta;
    onWidthChange(clampWidth(
      origin.width + widthDelta,
      minWidth,
      maxWidth,
    ));
  };

  const finishResize = (event: ReactPointerEvent<HTMLDivElement>) => {
    resizeOrigin.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  };

  const handleKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
    event.preventDefault();
    const direction = event.key === 'ArrowRight' ? 1 : -1;
    const sideDirection = side === 'left' ? direction : -direction;
    const step = event.shiftKey ? 24 : 8;
    onWidthChange(clampWidth(
      width + sideDirection * step,
      minWidth,
      maxWidth,
    ));
  };

  return (
    <div
      role="separator"
      aria-label={side === 'left' ? '调整左侧面板宽度' : '调整右侧面板宽度'}
      aria-orientation="vertical"
      aria-valuemin={minWidth}
      aria-valuemax={maxWidth}
      aria-valuenow={width}
      tabIndex={0}
      className={`group absolute inset-y-0 z-50 w-2 cursor-col-resize touch-none outline-none ${positionClassName}`}
      onDoubleClick={() => onWidthChange(defaultWidth)}
      onKeyDown={handleKeyDown}
      onPointerCancel={finishResize}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={finishResize}
    >
      <span className="absolute inset-y-0 left-1/2 w-px -translate-x-1/2 bg-transparent transition-colors group-hover:bg-blue-400 group-focus-visible:bg-blue-500" />
    </div>
  );
}
