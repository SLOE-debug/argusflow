import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { FlowCanvasTools } from './FlowCanvasTools';
import { createFlowStore, FlowProvider } from './store';

describe('FlowCanvasTools', () => {
  it('requests pan mode and reflects the controlled active tool', () => {
    const onModeChange = vi.fn();
    const store = createFlowStore();
    const view = render(
      <FlowProvider store={store}>
        <FlowCanvasTools
          mode="select"
          onModeChange={onModeChange}
        />
      </FlowProvider>,
    );

    fireEvent.click(screen.getByRole('button', { name: '平移' }));
    expect(onModeChange).toHaveBeenCalledWith('pan');

    view.rerender(
      <FlowProvider store={store}>
        <FlowCanvasTools
          mode="pan"
          onModeChange={onModeChange}
        />
      </FlowProvider>,
    );
    expect(screen.getByRole('button', { name: '平移' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
  });

  it('shows canvas shortcut guidance from the settings control', () => {
    const store = createFlowStore();
    render(
      <FlowProvider store={store}>
        <FlowCanvasTools
          mode="select"
          onModeChange={vi.fn()}
        />
      </FlowProvider>,
    );

    fireEvent.click(screen.getByRole('button', { name: '画布设置' }));

    expect(screen.getByRole('heading', { name: '画布快捷操作' })).toBeVisible();
    expect(screen.getByText('微调选中节点 1px')).toBeVisible();
  });
});
