import type { MouseButton, RawTraceEvent, RecordingTrace } from './model';
import type { ScreenPoint } from './inspectionContracts';

/** 保留的真实采样点，时间为录制相对毫秒，坐标为屏幕物理像素。 */
export type MotionPoint = Readonly<{ sequence: number; elapsed_ms: number; point: ScreenPoint }>;
/** 连续鼠标移动的事实摘要；不推断为 Workflow 拖拽动作。 */
export type PointerMotion = Readonly<{
  /** 该段最后一个原始事件序号。 */
  end_sequence: number;
  /** 最后一个原始采样的时间，毫秒。 */
  ended_ms: number;
  /** 合并前的采样数量。 */
  sample_count: number;
  /** 根据所有原始点计算的移动距离，物理像素。 */
  distance_px: number;
  /** 录制内观察到尚未释放的鼠标键。 */
  pressed_buttons: readonly MouseButton[];
  /** 2 像素误差内简化的轨迹，保留首尾和时间锚点。 */
  points: readonly MotionPoint[];
}>;

/** 界面与导出共享真实源采样数，合并不计入丢弃。 */
export function sourceEventCount(trace: RecordingTrace): number {
  return trace.timeline.events.reduce((count, event) => count + (event.input.type === 'pointer_motion' ? event.input.sample_count : 1), 0);
}

/** 移动段显示一行概要，避免每个采样点被当作独立操作。 */
export function motionLabel(motion: PointerMotion): string {
  const first = motion.points[0];
  const last = motion.points.at(-1);
  const label = motion.pressed_buttons.length > 0 ? '按住鼠标移动' : '鼠标移动';
  return first && last ? `${label} (${first.point.x}, ${first.point.y}) → (${last.point.x}, ${last.point.y})` : label;
}

/** 移动采样无需逐点反查窗口，不能将此描述为证据缺失故障。 */
export function eventContextLabel(event: RawTraceEvent): string {
  if (event.input.type === 'pointer_motion') {
    return `持续 ${((event.input.ended_ms - event.elapsed_ms) / 1000).toFixed(2)} 秒 · 合并 ${event.input.sample_count} 个采样点`;
  }
  if (event.input.type === 'move') return '单个移动采样点';
  return event.evidence?.context?.title ?? '窗口信息缺失';
}
