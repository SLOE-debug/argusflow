import { describe, expect, it, vi } from 'vitest';

import { createAnimationFrameCoalescer } from './pointerGesture';

describe('animation frame coalescer', () => {
  it('applies only the latest value once per animation frame', () => {
    let frameCallback: FrameRequestCallback | null = null;
    vi.stubGlobal('requestAnimationFrame', vi.fn((callback: FrameRequestCallback) => {
      frameCallback = callback;
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
    frameCallback?.(16);
    expect(apply).toHaveBeenCalledOnce();
    expect(apply).toHaveBeenCalledWith(3);

    vi.unstubAllGlobals();
  });
});
