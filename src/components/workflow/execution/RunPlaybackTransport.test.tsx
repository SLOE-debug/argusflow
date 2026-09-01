import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import type { ExecutionEvent } from '../../../features/workflow';
import { RunPlaybackTransport } from './RunPlaybackTransport';

describe('RunPlaybackTransport', () => {
  it('supports keyboard range navigation and exposes a readable event value', () => {
    const onCursorChange = vi.fn();
    const events: ExecutionEvent[] = [0, 1, 2].map((sequence) => ({
      run_id: 'run-1', workflow_id: 'workflow-1', sequence,
      node_id: 'node-1', edge_id: null, kind: 'node_started', message: null, payload: null,
    }));
    render(
      <RunPlaybackTransport
        events={events}
        cursor={1}
        presentation={{ schema_version: 1, node_labels: { 'node-1': '检查界面' } }}
        followLatest={false}
        currentSource
        onCursorChange={onCursorChange}
        onReturnToLatest={vi.fn()}
      />,
    );

    const slider = screen.getByRole('slider', { name: '运行事件时间线' });
    expect(slider).toHaveAttribute('aria-valuetext', '检查界面：开始执行');
    fireEvent.change(slider, { target: { value: '2' } });
    expect(onCursorChange).toHaveBeenCalledWith(2);
    expect(screen.getAllByTestId('run-event-tick')).toHaveLength(3);

    fireEvent.mouseEnter(screen.getByTestId('run-event-scale'));
    fireEvent.keyDown(window, { key: 'ArrowRight' });
    expect(onCursorChange).toHaveBeenCalledWith(2);
    fireEvent.keyDown(window, { key: 'ArrowLeft' });
    expect(onCursorChange).toHaveBeenCalledWith(0);

    expect(screen.getByRole('button', { name: '回到最新' })).toBeVisible();
  });
});
