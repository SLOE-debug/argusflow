import { useEffect, useMemo, useState } from 'react';
import type { RawTraceEvent } from './model';
import { latestAt, screenPrivacyFrames } from './privacyTimeline';
import type { ScreenTimeline } from './screenContracts';

/** 时间驱动的回看状态；播放只读取录制，不重放真实键鼠操作。 */
export function usePrivacyPlayback(events: readonly RawTraceEvent[], screen: ScreenTimeline) {
  const frames = useMemo(() => screenPrivacyFrames(screen), [screen]);
  const sources = useMemo(() => [...new Set(screen.frames.map((frame) => frame.source))], [screen]);
  const [source, setSource] = useState<number | null>(null);
  const selectedSource = source !== null && sources.includes(source) ? source : sources[0];
  const sourceFrames = useMemo(() => frames.filter((frame) => frame.source === selectedSource), [frames, selectedSource]);
  const duration = useMemo(() => events.reduce((end, event) => Math.max(end, event.input.type === 'pointer_motion' ? event.input.ended_ms : event.elapsed_ms), Math.max(screen.duration_us / 1000, frames.at(-1)?.shot.captured_at_ms ?? 0)), [events, frames, screen.duration_us]);
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
  const step = (direction: -1 | 1) => {
    const frame = direction > 0 ? sourceFrames.find((frame) => frame.shot.captured_at_ms > time) : [...sourceFrames].reverse().find((frame) => frame.shot.captured_at_ms < time);
    if (frame) seek(frame.shot.captured_at_ms);
  };
  return { frames, sources, selectedSource, setSource, step, duration, time, playing, seek, toggle, pause: () => setPlaying(false), frame: latestAt(sourceFrames, time, (frame) => frame.shot.captured_at_ms) };
}
