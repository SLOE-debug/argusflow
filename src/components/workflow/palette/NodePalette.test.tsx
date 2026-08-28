import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { createFlowStore, FLOW_NODE_KIND_DRAG_TYPE } from '../../../flow';
import type {
  WorkflowEdgeData,
  WorkflowNodeData,
} from '../../../features/workflow';
import { NodePalette } from './NodePalette';

describe('NodePalette', () => {
  it('publishes enabled node kinds through native drag data', () => {
    const setData = vi.fn();
    const dataTransfer = { effectAllowed: 'none', setData };

    const store = createFlowStore<WorkflowNodeData, WorkflowEdgeData>();
    render(<NodePalette store={store} onCollapse={vi.fn()} />);

    const triggerNode = screen.getByRole('button', { name: '开始' });
    expect(triggerNode).toHaveAttribute('draggable', 'true');

    fireEvent.dragStart(triggerNode, { dataTransfer });

    expect(dataTransfer.effectAllowed).toBe('copy');
    expect(setData).toHaveBeenCalledWith(FLOW_NODE_KIND_DRAG_TYPE, 'start');

    const actionNode = screen.getByRole('button', { name: '操作界面' });
    fireEvent.dragStart(actionNode, { dataTransfer });
    expect(setData).toHaveBeenCalledWith(FLOW_NODE_KIND_DRAG_TYPE, 'ui');

    const debugNode = screen.getByRole('button', { name: '查看结果' });
    fireEvent.dragStart(debugNode, { dataTransfer });
    expect(setData).toHaveBeenCalledWith(FLOW_NODE_KIND_DRAG_TYPE, 'debug');
    expect(setData).toHaveBeenCalledWith('text/plain', 'argusflow-node:debug');
  });

  it('collapses and expands node groups', () => {
    const store = createFlowStore<WorkflowNodeData, WorkflowEdgeData>();
    render(<NodePalette store={store} onCollapse={vi.fn()} />);

    const triggerGroup = screen.getByRole('button', { name: /^流程控制/ });
    fireEvent.click(triggerGroup);
    expect(screen.queryByRole('button', { name: '开始' })).not.toBeInTheDocument();
    expect(triggerGroup).toHaveAttribute('aria-expanded', 'false');

    fireEvent.click(triggerGroup);
    expect(screen.getByRole('button', { name: '开始' })).toBeInTheDocument();
    expect(triggerGroup).toHaveAttribute('aria-expanded', 'true');
  });

  it('shows only creatable nodes and opens module placeholders', () => {
    const store = createFlowStore<WorkflowNodeData, WorkflowEdgeData>();
    const onCollapse = vi.fn();
    render(<NodePalette store={store} onCollapse={onCollapse} />);

    expect(screen.queryByRole('button', { name: '定时触发' })).not.toBeInTheDocument();
    expect(screen.getByText('操作界面', { selector: 'strong' })).toHaveAttribute(
      'title',
      '操作界面',
    );
    expect(screen.getByText('点击、输入或读取界面内容')).toHaveAttribute(
      'title',
      '点击、输入或读取界面内容',
    );
    fireEvent.click(screen.getByRole('button', { name: '收起左侧面板' }));
    expect(onCollapse).toHaveBeenCalledOnce();

    fireEvent.click(screen.getByRole('button', { name: '预设' }));
    expect(screen.getByRole('button', { name: '发送微信群消息' })).toHaveAttribute(
      'draggable',
      'true',
    );
    expect(screen.getByText('流程组件', { selector: 'h3' })).toBeVisible();

    fireEvent.click(screen.getByRole('button', { name: '资源' }));
    expect(screen.getByText('管理流程要用的数据和凭据。')).toBeVisible();
    expect(screen.queryByRole('textbox', { name: '搜索节点' })).not.toBeInTheDocument();
  });
});
