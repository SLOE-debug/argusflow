import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { WorkflowWorkspace } from './WorkflowWorkspace';

describe('WorkflowWorkspace', () => {
  it('renders the editor without a duplicated document tab', () => {
    render(
      <WorkflowWorkspace
        canvas={<div>画布内容</div>}
        dockOpen={false}
        editorState={{ target: null, mode: 'docked', dockHeight: 320 }}
        events={[]}
        nodes={[]}
        report={null}
        onDockOpenChange={vi.fn()}
        onDockHeightChange={vi.fn()}
        onEditorModeChange={vi.fn()}
        onCloseEditor={vi.fn()}
        onUpdateNode={vi.fn()}
      />,
    );

    expect(screen.queryByRole('button', { name: '关闭工作流页签' })).not.toBeInTheDocument();
    expect(screen.getByText('画布内容')).toBeVisible();
    expect(screen.queryByRole('button', { name: '打开工作流概览' })).not.toBeInTheDocument();
  });
});
