import { diagnosticLabel, type PointerMotion, type RawTraceEvent } from '../../features/recorder';

/** 鼠标路径详情展示移动事实与采样压缩结果，避免空白 UIA/截图失败提示。 */
export function PointerMotionDetails({ event, motion }: Readonly<{ event: RawTraceEvent; motion: PointerMotion }>) {
  const first = motion.points[0];
  const last = motion.points.at(-1);
  const facts = [
    ['原始事件', `${event.sequence}–${motion.end_sequence}`],
    ['时间范围', `${event.elapsed_ms}–${motion.ended_ms} ms`],
    ['持续时间', `${((motion.ended_ms - event.elapsed_ms) / 1000).toFixed(2)} 秒`],
    ['采样合并', `${motion.sample_count} 个采样点 → ${motion.points.length} 个轨迹点`],
    ['起点', first ? `(${first.point.x}, ${first.point.y})` : '缺失'],
    ['终点', last ? `(${last.point.x}, ${last.point.y})` : '缺失'],
    ['移动距离', `${Math.round(motion.distance_px)} 像素`],
    ['按下的鼠标键', motion.pressed_buttons.length ? motion.pressed_buttons.map((button) => BUTTON_LABELS[button]).join('、') : '未观察到按下'],
  ] as const;
  return (
    <article className="min-w-0 space-y-4 p-4">
      <h3 className="text-sm font-semibold">鼠标移动轨迹</h3>
      <dl className="grid grid-cols-[7rem_minmax(0,1fr)] gap-x-3 gap-y-2 text-xs">
        {facts.map(([label, value]) => (
          <div
            key={label}
            className="contents"
          >
            <dt className="text-slate-500">{label}</dt>
            <dd className="min-w-0 break-words text-slate-800">{value}</dd>
          </div>
        ))}
      </dl>
      <p className="text-xs leading-5 text-slate-500">保留起终点、关键转折和时间锚点；点击、按键、窗口切换与停顿会分开记录。</p>
      {event.diagnostics.map((diagnostic, index) => (
        <p
          key={index}
          className="text-xs text-amber-800"
        >{diagnosticLabel(diagnostic)}</p>
      ))}
      <details className="text-xs">
        <summary className="cursor-pointer text-slate-600">轨迹采样详情</summary>
        <pre className="mt-2 overflow-auto rounded-md bg-slate-50 p-2">{JSON.stringify(event, null, 2)}</pre>
      </details>
    </article>
  );
}

/** 鼠标原始键名的展示映射，不推断高级操作。 */
const BUTTON_LABELS = { left: '左键', right: '右键', middle: '中键', x1: '侧键 1', x2: '侧键 2' } as const;
