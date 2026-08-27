/** 一次指针手势在窗口级监听期间需要执行的回调。 */
export type PointerGestureHandlers = Readonly<{
  /** 指针移动时接收最新原生事件。 */
  move: (event: PointerEvent) => void;
  /** 正常抬起时提交手势。 */
  finish: (event: PointerEvent) => void;
  /** 浏览器取消指针时清理临时状态。 */
  cancel?: (event: PointerEvent) => void;
}>;

/** 仅在当前手势期间绑定全局监听，并在结束或取消后完整解绑。 */
export function bindPointerGesture({
  move,
  finish,
  cancel,
}: PointerGestureHandlers): void {
  /** 移除当前手势创建的全部窗口监听。 */
  const cleanup = () => {
    window.removeEventListener('pointermove', move);
    window.removeEventListener('pointerup', handlePointerUp);
    window.removeEventListener('pointercancel', handlePointerCancel);
  };
  /** 正常抬起时先解绑，再提交最终位置。 */
  const handlePointerUp = (event: PointerEvent) => {
    cleanup();
    finish(event);
  };
  /** 指针被系统取消时不提交历史，只执行清理回调。 */
  const handlePointerCancel = (event: PointerEvent) => {
    cleanup();
    cancel?.(event);
  };

  window.addEventListener('pointermove', move);
  window.addEventListener('pointerup', handlePointerUp);
  window.addEventListener('pointercancel', handlePointerCancel);
}

/** 把连续输入压缩为每个动画帧最多一次处理，并始终保留最新值。 */
export type AnimationFrameCoalescer<T> = Readonly<{
  /** 等待下一动画帧处理该值；同帧内旧值会被替换。 */
  schedule: (value: T) => void;
  /** 立即处理最终值，并取消尚未执行的帧。 */
  flush: (value: T) => void;
  /** 丢弃待处理值并取消尚未执行的帧。 */
  cancel: () => void;
}>;

/** 创建适用于指针移动、框选和临时连线的动画帧合并器。 */
export function createAnimationFrameCoalescer<T>(
  apply: (value: T) => void,
): AnimationFrameCoalescer<T> {
  /** 当前等待执行的动画帧 ID。 */
  let frameId: number | null = null;
  /** 同一帧内收到的最后一个输入值。 */
  let latestValue: T | null = null;

  /** 处理当前帧保存的最新输入。 */
  const run = () => {
    const value = latestValue;
    frameId = null;
    latestValue = null;
    if (value !== null) apply(value);
  };
  /** 取消待执行帧并清空输入。 */
  const cancel = () => {
    if (frameId !== null) cancelAnimationFrame(frameId);
    frameId = null;
    latestValue = null;
  };

  return {
    schedule: (value) => {
      latestValue = value;
      frameId ??= requestAnimationFrame(run);
    },
    flush: (value) => {
      cancel();
      apply(value);
    },
    cancel,
  };
}
