import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { WorkspaceDockPanel } from './WorkspaceDockPanel';

describe('WorkspaceDockPanel', () => {
  it('keeps utility tabs distinct and hosts a non-modal structured document tab', () => {
    const onEditorModeChange = vi.fn();
    const onOpenRunWorkbench = vi.fn();
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
                  query: { language_version: 3 as const, bindings: {}, source: 'button()' },
                },
                backend_policy: { allow: [], deny: [], prefer: [] },
              },
            },
            execution: {
              target_wait: { mode: 'bounded', timeout_ms: 5_000, poll_interval_ms: 100 },
            },
          },
        }]}
        report={null}
        onOpenChange={vi.fn()}
        onDockHeightChange={vi.fn()}
        onEditorModeChange={onEditorModeChange}
        onCloseEditor={vi.fn()}
        onOpenRunWorkbench={onOpenRunWorkbench}
      />,
    );

    expect(screen.getByText('AQL 编辑器内容')).toBeVisible();
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    const documentTab = screen.getByText('AQL').closest('button');
    if (!documentTab) throw new Error('AQL 文档页签未渲染。');
    expect(documentTab).toHaveClass('h-full', 'items-center', 'leading-none');
    for (const titlePart of documentTab.querySelectorAll('span')) {
      expect(titlePart).toHaveClass('h-full', 'items-center', 'leading-none');
    }
    fireEvent.click(screen.getByRole('button', { name: '最大化编辑器' }));
    expect(onEditorModeChange).toHaveBeenCalledWith('maximized');

    fireEvent.click(screen.getByRole('button', { name: '运行记录' }));
    expect(onOpenRunWorkbench).toHaveBeenCalledOnce();
    expect(screen.getByRole('heading', { name: '运行记录已在执行台打开' })).toBeVisible();
    expect(screen.queryByRole('button', { name: '运行日志' })).not.toBeInTheDocument();
  });
});
