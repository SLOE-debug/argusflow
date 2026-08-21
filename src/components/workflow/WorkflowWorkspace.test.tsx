import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { WorkflowWorkspace } from './WorkflowWorkspace';

describe('WorkflowWorkspace', () => {
  it('renders the editor without a duplicated document tab', () => {
    render(
      <WorkflowWorkspace
        canvas={<div>画布内容</div>}
        open={false}
        events={[]}
        report={null}
        onToggle={vi.fn()}
      />,
    );

    expect(screen.queryByRole('button', { name: '关闭工作流页签' })).not.toBeInTheDocument();
    expect(screen.getByText('画布内容')).toBeVisible();
    expect(screen.queryByRole('button', { name: '打开工作流概览' })).not.toBeInTheDocument();
  });
});
