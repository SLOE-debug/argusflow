import { useEffect, useRef } from 'react';
import { Timeline } from '../../ui';

import type { ExecutionEvent } from '../../../features/workflow';

type RunEventScaleProps = Readonly<{
  events: ReadonlyArray<ExecutionEvent>;
  cursor: number;
  valueText: string;
  onCursorChange: (cursor: number) => void;
}>;

/**
 * 将运行事件适配到通用时间轴；保留工作流悬停方向键导航。
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
      className="relative rounded-lg focus-within:ring-2 focus-within:ring-blue-500/30"
      onMouseEnter={() => { hoveredRef.current = true; }}
      onMouseLeave={() => { hoveredRef.current = false; }}
    >
      <Timeline
        label="运行事件时间线"
        maximum={Math.max(0, eventCount - 1)}
        value={safeCursor}
        valueText={valueText}
        disabled={eventCount === 0}
        markers={events.map((event, index) => ({ id: String(event.sequence) + ':' + index, value: index, label: '事件 ' + (index + 1) }))}
        format={(value) => String(Math.round(value) + 1)}
        onChange={onCursorChange}
      />
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
