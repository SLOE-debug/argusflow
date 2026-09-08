import { useEffect, useMemo, useRef } from 'react';
import { chordLabel, latestAt, type PrivacyFrame, type RawTraceEvent } from '../../features/recorder';

/** 播放键帽通过 Web Animations 入场；新事件到达时旧键帽上移淡出。 */
function KeyCaption({ label, previous }: Readonly<{ label: string; previous: boolean }>) {
  const element = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (window.matchMedia?.('(prefers-reduced-motion: reduce)').matches) return;
    const animation = element.current?.animate?.(previous
      ? [{ transform: 'translateY(0)', opacity: 1 }, { transform: 'translateY(-42px)', opacity: 0 }]
      : [{ transform: 'translateY(20px)', opacity: 0 }, { transform: 'translateY(0)', opacity: 1 }],
    { duration: previous ? 500 : 220, fill: 'forwards', easing: 'ease-out' });
    return () => animation?.cancel();
  }, [previous]);
  return <div ref={element} className={`absolute bottom-0 left-1/2 max-w-[90%] -translate-x-1/2 truncate rounded-xl border border-white/30 bg-slate-950/85 px-5 py-2 text-lg font-medium text-white shadow-lg ${previous ? 'opacity-0' : ''}`}>{label}</div>;
}

/** 键鼠提示只是展示；屏幕坐标根据实际截图范围换算，范围外的指针不显示。 */
export function PrivacyPlaybackOverlay({ events, time, frame }: Readonly<{ events: readonly RawTraceEvent[]; time: number; frame: PrivacyFrame }>) {
  const keyEvents = useMemo(() => events.filter((event) => event.input.type === 'key' && event.input.phase === 'down'), [events]);
  const pointerEvents = useMemo(() => events.filter((event) => ['mouse', 'move', 'wheel', 'pointer_motion'].includes(event.input.type)), [events]);
  const keys = keyEvents.filter((event) => event.elapsed_ms <= time && time - event.elapsed_ms < 1800).slice(-2);
  const pointerEvent = latestAt(pointerEvents, time, (event) => event.elapsed_ms);
  const input = pointerEvent?.input;
  const pointer = input?.type === 'pointer_motion' ? latestAt(input.points, time, (point) => point.elapsed_ms)?.point
    : input && 'point' in input ? input.point : frame.shot.pointer;
  const bounds = frame.shot.screen_bounds;
  const x = pointer ? (pointer.x - bounds.x) / bounds.width * 100 : -1;
  const y = pointer ? (pointer.y - bounds.y) / bounds.height * 100 : -1;
  const clicking = input?.type === 'mouse' && input.phase === 'down' && time - (pointerEvent?.elapsed_ms ?? 0) < 650;
  return (
    <div className="pointer-events-none absolute inset-0 overflow-hidden" aria-hidden="true">
      {x >= 0 && x <= 100 && y >= 0 && y <= 100 ? (
        <div className={`absolute -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white shadow-md ${clicking ? 'size-9 bg-amber-400/60 ring-4 ring-amber-400/30' : 'size-3 bg-blue-600'}`} style={{ left: `${x}%`, top: `${y}%` }}>
          {clicking ? <span className="absolute left-8 top-0 whitespace-nowrap rounded bg-slate-950/80 px-2 py-1 text-xs text-white">{input.button === 'right' ? '右键点击' : '点击'}</span> : null}
          {input?.type === 'wheel' && time - (pointerEvent?.elapsed_ms ?? 0) < 650 ? <span className="absolute left-4 whitespace-nowrap rounded bg-slate-950/80 px-2 text-white">{input.horizontal ? '↔' : input.delta > 0 ? '↑ 滚动' : '↓ 滚动'}</span> : null}
        </div>
      ) : null}
      <div className="absolute inset-x-0 bottom-5 h-20">
        {keys.map((event, index) => <KeyCaption key={event.sequence} previous={index < keys.length - 1} label={event.input.type === 'key' ? event.input.chord ? chordLabel(event.input.chord) : event.input.text?.type === 'plain' ? event.input.text.value : '按键' : ''} />)}
      </div>
    </div>
  );
}
