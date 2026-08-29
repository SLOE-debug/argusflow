import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { WorkspaceDockPanel } from './WorkspaceDockPanel';

describe('WorkspaceDockPanel', () => {
  it('keeps utility tabs distinct and hosts a non-modal structured document tab', () => {
    const onEditorModeChange = vi.fn();
    render(
      <WorkspaceDockPanel
        open
        editorTarget={{ type: 'aql', nodeId: 'ui-1' }}
        editorMode="docked"
        dockHeight={320}
        minDockHeight={220}
        maxDockHeight={600}
        defaultDockHeight={320}
        structuredEditor={<div>AQL 编辑器内容</div>}
        events={[]}
        nodes={[{
          id: 'ui-1',
          kind: 'ui',
          position: { x: 0, y: 0 },
          size: { width: 164, height: 52 },
          data: {
            kind: 'ui',
            label: '保存按钮',
            outputBindings: {},
            operation: {
              type: 'click',
              target: {
                scope: { type: 'current' },
                locator: {
                  type: 'query',
                  query: { language_version: 1, source: 'button()' },
                },
                backend_policy: { allow: [], deny: [], prefer: [] },
              },
            },
            execution: {
              target_wait: { mode: 'bounded', timeout_ms: 5_000, poll_interval_ms: 100 },
              postcondition_wait: { mode: 'none', timeout_ms: 0, poll_interval_ms: 0 },
              postcondition: null,
            },
          },
        }]}
        report={null}
        onOpenChange={vi.fn()}
        onDockHeightChange={vi.fn()}
        onEditorModeChange={onEditorModeChange}
        onCloseEditor={vi.fn()}
      />,
    );

    expect(screen.getByText('AQL 编辑器内容')).toBeVisible();
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '最大化编辑器' }));
    expect(onEditorModeChange).toHaveBeenCalledWith('maximized');

    fireEvent.click(screen.getByRole('button', { name: '运行记录' }));
    expect(screen.getByRole('heading', { name: '运行记录' })).toBeVisible();
    expect(screen.getByRole('heading', { name: '时间线' })).toBeVisible();
    expect(screen.getByRole('heading', { name: '详情' })).toBeVisible();
    fireEvent.click(screen.getByRole('button', { name: '运行日志' }));
    expect(screen.getByRole('heading', { name: '运行日志' })).toBeVisible();
  });
});
