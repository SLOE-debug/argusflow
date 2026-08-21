import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { PanelResizeHandle } from './PanelResizeHandle';

describe('PanelResizeHandle', () => {
  it('resizes a left panel with pointer movement and clamps its minimum', () => {
    const onWidthChange = vi.fn();
    render(
      <PanelResizeHandle
        side="left"
        width={216}
        minWidth={176}
        maxWidth={360}
        defaultWidth={216}
        onWidthChange={onWidthChange}
      />,
    );
    const handle = screen.getByRole('separator', { name: '调整左侧面板宽度' });
    Object.assign(handle, {
      setPointerCapture: vi.fn(),
      hasPointerCapture: () => true,
      releasePointerCapture: vi.fn(),
    });

    fireEvent.pointerDown(handle, { button: 0, clientX: 200, pointerId: 1 });
    fireEvent.pointerMove(handle, { clientX: 100, pointerId: 1 });

    expect(onWidthChange).toHaveBeenLastCalledWith(176);
  });

  it('supports keyboard resizing and double-click reset', () => {
    const onWidthChange = vi.fn();
    render(
      <PanelResizeHandle
        side="right"
        width={280}
        minWidth={240}
        maxWidth={420}
        defaultWidth={280}
        onWidthChange={onWidthChange}
      />,
    );
    const handle = screen.getByRole('separator', { name: '调整右侧面板宽度' });

    fireEvent.keyDown(handle, { key: 'ArrowLeft' });
    expect(onWidthChange).toHaveBeenLastCalledWith(288);
    fireEvent.doubleClick(handle);
    expect(onWidthChange).toHaveBeenLastCalledWith(280);
  });
});
