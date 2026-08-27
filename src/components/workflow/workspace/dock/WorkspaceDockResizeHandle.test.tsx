import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { WorkspaceDockResizeHandle } from './WorkspaceDockResizeHandle';

describe('WorkspaceDockResizeHandle', () => {
  it('supports keyboard resizing, clamping and double-click reset', () => {
    const onHeightChange = vi.fn();
    render(
      <WorkspaceDockResizeHandle
        height={225}
        minHeight={220}
        maxHeight={400}
        defaultHeight={320}
        onHeightChange={onHeightChange}
      />,
    );
    const separator = screen.getByRole('separator', {
      name: '调整底部面板高度',
    });

    fireEvent.keyDown(separator, { key: 'ArrowDown' });
    expect(onHeightChange).toHaveBeenLastCalledWith(220);

    fireEvent.keyDown(separator, { key: 'ArrowUp', shiftKey: true });
    expect(onHeightChange).toHaveBeenLastCalledWith(265);

    fireEvent.doubleClick(separator);
    expect(onHeightChange).toHaveBeenLastCalledWith(320);
  });
});
