import { useEffect, useRef } from 'react';

import type { ExecutionEvent } from '../../../features/workflow';

type RunEventScaleProps = Readonly<{
  events: ReadonlyArray<ExecutionEvent>;
  cursor: number;
  valueText: string;
  onCursorChange: (cursor: number) => void;
}>;

/**
 * 用可拖拽的原生 range 承载输入语义，并在其下绘制运行事件刻度与当前位置标记。
 * 鼠标停留在刻度区域时，左右方向键也可以逐事件回放，而不需要先点击取得焦点。
 */
export function RunEventScale({
  events,
  cursor,
  valueText,
  onCursorChange,
}: RunEventScaleProps) {
  const scaleRef = useRef<HTMLDivElement>(null);
  const hoveredRef = useRef(false);
  const eventCount = events.length;
  const safeCursor = eventCount === 0 ? 0 : Math.min(Math.max(cursor, 0), eventCount - 1);
  const progress = eventCount <= 1 ? 0 : (safeCursor / (eventCount - 1)) * 100;

  useEffect(() => {
    /** 悬停快捷键不能影响正在编辑文字的表单或可编辑区域。 */
    const handleWindowKeyDown = (event: KeyboardEvent) => {
      if (!hoveredRef.current || eventCount === 0 || isEditableTarget(event.target)) return;
      if (event.target instanceof Node && scaleRef.current?.contains(event.target)) return;
      if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;

      event.preventDefault();
      const direction = event.key === 'ArrowLeft' ? -1 : 1;
      const nextCursor = Math.min(Math.max(safeCursor + direction, 0), eventCount - 1);
      if (nextCursor !== safeCursor) onCursorChange(nextCursor);
    };

    window.addEventListener('keydown', handleWindowKeyDown);
    return () => window.removeEventListener('keydown', handleWindowKeyDown);
  }, [eventCount, onCursorChange, safeCursor]);

  return (
    <div
      ref={scaleRef}
      data-testid="run-event-scale"
      className="relative h-11 rounded-lg px-2 focus-within:ring-2 focus-within:ring-blue-500/30"
      onMouseEnter={() => { hoveredRef.current = true; }}
      onMouseLeave={() => { hoveredRef.current = false; }}
    >
      <div className="pointer-events-none absolute inset-x-2 top-[29px] h-px bg-slate-300" />
      <div className="pointer-events-none absolute inset-x-2 top-[24px] h-3">
        {events.map((event, index) => {
          const tickProgress = eventCount <= 1 ? 0 : (index / (eventCount - 1)) * 100;
          const isMajorTick = index === 0 || index === eventCount - 1 || index % 5 === 0;
          return (
            <span
              key={`${event.sequence}-${index}`}
              data-testid="run-event-tick"
              className={
                'absolute top-0 w-px -translate-x-1/2 ' +
                (isMajorTick ? 'h-3' : 'h-2') +
                (index <= safeCursor ? ' bg-blue-500' : ' bg-slate-300')
              }
              style={{ left: `${tickProgress}%` }}
            />
          );
        })}
      </div>
      {eventCount > 0 ? (
        <div className="pointer-events-none absolute inset-x-2 top-0 z-10" aria-hidden="true">
          <div
            className="absolute top-0 -translate-x-1/2"
            style={{ left: `${progress}%` }}
          >
            <div className="min-w-7 rounded border border-blue-700 bg-blue-600 px-1.5 py-0.5 text-center text-[11px] font-semibold leading-4 text-white shadow-sm">
              {safeCursor + 1}
            </div>
            <div className="mx-auto h-0 w-0 border-x-[5px] border-t-[6px] border-x-transparent border-t-blue-600" />
          </div>
        </div>
      ) : null}
      {/* 透明 range 保留拖拽、点击、触摸和获得焦点后的原生键盘语义。 */}
      <input
        type="range"
        aria-label="运行事件时间线"
        aria-valuetext={valueText}
        aria-describedby="run-event-scale-help"
        className="absolute inset-0 z-20 h-full w-full cursor-pointer opacity-0 disabled:cursor-not-allowed"
        min={0}
        max={Math.max(0, eventCount - 1)}
        value={safeCursor}
        disabled={eventCount === 0}
        onChange={(event) => onCursorChange(Number(event.currentTarget.value))}
      />
      <span id="run-event-scale-help" className="sr-only">
        拖动或使用左右方向键切换运行事件。
      </span>
    </div>
  );
}

/** 判断键盘事件是否来自应保留方向键行为的文字编辑控件。 */
function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.isContentEditable
    || target.tagName === 'INPUT'
    || target.tagName === 'TEXTAREA'
    || target.tagName === 'SELECT';
}
