import { describe, expect, it, vi } from 'vitest';

import { createAnimationFrameCoalescer } from './pointerGesture';

describe('animation frame coalescer', () => {
  it('applies only the latest value once per animation frame', () => {
    /** 通过可变对象跨 mock 回调保存浏览器安排的帧函数。 */
    const pendingFrame: { callback: FrameRequestCallback | null } = { callback: null };
    vi.stubGlobal('requestAnimationFrame', vi.fn((callback: FrameRequestCallback) => {
      pendingFrame.callback = callback;
      return 1;
    }));
    vi.stubGlobal('cancelAnimationFrame', vi.fn());
    const apply = vi.fn();
    const frames = createAnimationFrameCoalescer(apply);

    frames.schedule(1);
    frames.schedule(2);
    frames.schedule(3);

    expect(requestAnimationFrame).toHaveBeenCalledOnce();
    expect(apply).not.toHaveBeenCalled();
    pendingFrame.callback?.(16);
    expect(apply).toHaveBeenCalledOnce();
    expect(apply).toHaveBeenCalledWith(3);

    vi.unstubAllGlobals();
  });
});
