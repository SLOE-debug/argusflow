import { useEffect, useMemo, useState } from 'react';
import type { RawTraceEvent } from './model';
import { latestAt, privacyFrames } from './privacyTimeline';

/** 时间驱动的回看状态；播放只读取录制，不重放真实键鼠操作。 */
export function usePrivacyPlayback(events: readonly RawTraceEvent[]) {
  const frames = useMemo(() => privacyFrames(events), [events]);
  const duration = useMemo(() => events.reduce((end, event) => Math.max(end, event.input.type === 'pointer_motion' ? event.input.ended_ms : event.elapsed_ms), frames.at(-1)?.shot.captured_at_ms ?? 0), [events, frames]);
  const [time, setTime] = useState(0);
  const [playing, setPlaying] = useState(false);
  useEffect(() => { setTime((value) => Math.min(value, duration)); setPlaying(false); }, [events, duration]);
  useEffect(() => {
    if (!playing) return;
    const started = performance.now() - time;
    let request = 0;
    const tick = () => {
      const next = Math.min(duration, performance.now() - started);
      setTime(next);
      if (next >= duration) setPlaying(false);
      else request = requestAnimationFrame(tick);
    };
    request = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(request);
    // 播放起点只在切换播放状态时固定，时间更新不重启时钟。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [playing, duration]);
  const seek = (next: number) => { setPlaying(false); setTime(Math.max(0, Math.min(duration, next))); };
  const toggle = () => { if (!playing && time >= duration) setTime(0); setPlaying((value) => !value); };
  return { frames, duration, time, playing, seek, toggle, pause: () => setPlaying(false), frame: latestAt(frames, time, (frame) => frame.shot.captured_at_ms) };
}
