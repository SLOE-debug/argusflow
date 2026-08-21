import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { WorkflowConsolePanel } from './WorkflowConsolePanel';

describe('WorkflowConsolePanel', () => {
  it('renders distinct content for unfinished tabs instead of reusing logs', () => {
    render(
      <WorkflowConsolePanel
        open
        events={[]}
        report={null}
        onToggle={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: '运行记录' }));
    expect(screen.getByRole('heading', { name: '暂无运行记录' })).toBeVisible();

    fireEvent.click(screen.getByRole('button', { name: '告警' }));
    expect(screen.getByRole('heading', { name: '暂无告警' })).toBeVisible();

    fireEvent.click(screen.getByRole('button', { name: '日志' }));
    expect(screen.getByRole('heading', { name: '执行日志' })).toBeVisible();
  });
});
