import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import type { ExecutionEvent } from '../../features/workflow/contracts';
import { ExecutionLog } from './ExecutionLog';

/** 验证执行事件和结构校验问题能同时呈现在日志面板中。 */
describe('ExecutionLog', () => {
  it('renders workflow events and validation issues', () => {
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
  });
});
