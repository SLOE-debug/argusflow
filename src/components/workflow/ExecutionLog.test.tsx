import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import type { ExecutionEvent } from '../../features/workflow/contracts';
import { ExecutionLog } from './ExecutionLog';

/** 验证执行事件和结构校验问题能同时呈现在日志面板中。 */
describe('ExecutionLog', () => {
  it('renders workflow events and validation issues', () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText },
    });
    /** 代表一次 Log 节点事件的最小后端事件负载。 */
    const events: ExecutionEvent[] = [
      {
        run_id: 'run',
        workflow_id: 'workflow',
        sequence: 2,
        node_id: 'log',
        edge_id: null,
        kind: 'log',
        message: 'ArgusFlow 已启动',
        payload: null,
      },
    ];

    render(
      <ExecutionLog
        events={events}
        report={{
          valid: false,
          issues: [
            {
              code: 'invalid_node_degree',
              message: '首版只支持线性流程',
              node_id: 'log',
              edge_id: null,
            },
          ],
        }}
      />,
    );

    expect(screen.getByText(/ArgusFlow 已启动/)).toBeInTheDocument();
    expect(screen.getByText(/首版只支持线性流程/)).toBeInTheDocument();
    expect(screen.getByText(/ArgusFlow 已启动/)).not.toHaveClass('truncate');

    fireEvent.click(screen.getByRole('button', { name: '复制完整执行日志' }));
    expect(writeText).toHaveBeenCalledWith('02 log [log] ArgusFlow 已启动');
  });
});
