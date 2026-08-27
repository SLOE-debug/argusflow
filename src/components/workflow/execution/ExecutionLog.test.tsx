import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import type { ExecutionEvent } from '../../../features/workflow';
import type { WorkflowCanvasNode } from '../../../features/workflow';
import { ExecutionLog } from './ExecutionLog';

/** 验证执行事件和结构校验问题能同时呈现在日志面板中。 */
describe('ExecutionLog', () => {
  it('renders localized workflow events and validation issues', () => {
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
        sequence: 1,
        node_id: 'log',
        edge_id: null,
        kind: 'node_started',
        message: null,
        payload: null,
      },
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
      {
        run_id: 'run',
        workflow_id: 'workflow',
        sequence: 3,
        node_id: 'log',
        edge_id: null,
        kind: 'backend_selected',
        message: 'windows_uia',
        payload: { type: 'backend_selected', backend: 'windows_uia' },
      },
    ];
    const nodes: WorkflowCanvasNode[] = [{
      id: 'log',
      kind: 'log',
      position: { x: 0, y: 0 },
      size: { width: 142, height: 52 },
      data: {
        kind: 'log',
        label: '记录启动状态',
        outputBindings: {},
        message: 'ArgusFlow 已启动',
      },
    }];

    render(
      <ExecutionLog
        events={events}
        nodes={nodes}
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
    expect(screen.getByText('开始执行')).toBeVisible();
    expect(screen.getByText('已选择执行方式')).toBeVisible();
    expect(screen.getByText('Windows UI 自动化')).toBeVisible();
    expect(screen.queryByText('node_started')).not.toBeInTheDocument();
    expect(screen.getAllByText('记录启动状态')).toHaveLength(3);
    expect(document.querySelectorAll('[data-node-tone="log"]')).toHaveLength(3);

    fireEvent.click(screen.getByRole('button', { name: '复制运行日志' }));
    expect(writeText).toHaveBeenCalledWith([
      '01 [记录启动状态] 开始执行 正在执行',
      '02 [记录启动状态] 记录信息 ArgusFlow 已启动',
      '03 [记录启动状态] 已选择执行方式 Windows UI 自动化',
    ].join('\n'));
  });
});
