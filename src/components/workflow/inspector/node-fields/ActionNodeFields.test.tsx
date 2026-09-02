import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import {
  EMPTY_WORKFLOW_RESOURCE_CATALOG,
  type UiOperation,
} from '../../../../features/workflow';
import { ActionNodeFields } from './ActionNodeFields';

describe('ActionNodeFields', () => {
  it('opens the AQL document in Workspace without mounting an Inspector editor', () => {
    const onChange = vi.fn();
    const onOpenEditor = vi.fn();
    const operation: UiOperation = {
      type: 'click',
      target: {
        scope: { type: 'current' },
        locator: {
          type: 'query',
          query: { language_version: 3 as const, bindings: {}, source: 'button(name = "保存")' },
        },
        backend_policy: {
          allow: ['windows_uia'],
          deny: [],
          prefer: ['windows_uia'],
        },
      },
    };

    render(
      <ActionNodeFields
        nodeId="ui-save"
        operation={operation}
        execution={{
          target_wait: { mode: 'bounded', timeout_ms: 5_000, poll_interval_ms: 100 },
        }}
        resourceCatalog={EMPTY_WORKFLOW_RESOURCE_CATALOG}
        onChange={onChange}
        onExecutionChange={vi.fn()}
        onOpenEditor={onOpenEditor}
      />,
    );

    expect(screen.queryByRole('textbox', { name: 'AQL 查找条件' })).not.toBeInTheDocument();
    expect(screen.queryByText('button(name = "保存")')).not.toBeInTheDocument();
    expect(screen.queryByText('更多设置')).not.toBeInTheDocument();
    expect(screen.getByText('执行方式')).toBeVisible();
    expect(screen.queryByText(/运行环境/)).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '编辑查找条件' }));
    expect(onOpenEditor).toHaveBeenCalledWith({ type: 'aql', nodeId: 'ui-save' });
    expect(onChange).not.toHaveBeenCalled();
  });

  it('edits the node-owned target wait policy without adding another selector', () => {
    const onExecutionChange = vi.fn();
    const operation: UiOperation = {
      type: 'click',
      target: {
        scope: { type: 'current' },
        locator: {
          type: 'query',
          query: { language_version: 3 as const, bindings: {}, source: 'button(name = "继续")' },
        },
        backend_policy: { allow: [], deny: [], prefer: [] },
      },
    };

    render(
      <ActionNodeFields
        nodeId="ui-continue"
        operation={operation}
        execution={{
          target_wait: { mode: 'bounded', timeout_ms: 5_000, poll_interval_ms: 100 },
        }}
        resourceCatalog={EMPTY_WORKFLOW_RESOURCE_CATALOG}
        onChange={vi.fn()}
        onExecutionChange={onExecutionChange}
        onOpenEditor={vi.fn()}
      />,
    );

    fireEvent.change(screen.getByRole('spinbutton', { name: '最长等待目标时间' }), {
      target: { value: '8000' },
    });
    expect(onExecutionChange).toHaveBeenCalledWith({
      target_wait: { mode: 'bounded', timeout_ms: 8_000, poll_interval_ms: 100 },
    });

    expect(screen.getAllByText('毫秒')).toHaveLength(2);
    fireEvent.click(screen.getByRole('checkbox', { name: '找不到目标时等待' }));
    expect(onExecutionChange).toHaveBeenLastCalledWith({
      target_wait: { mode: 'none', timeout_ms: 0, poll_interval_ms: 0 },
    });
    expect(screen.queryByText('button(name = "继续")')).not.toBeInTheDocument();
  });

  it('selects a guaranteed application node and disables unsafe references', () => {
    const onChange = vi.fn();
    const operation: UiOperation = {
      type: 'click',
      target: {
        scope: {
          type: 'application',
          resource: { producer_node_id: 'app-after', output_name: 'session' },
        },
        locator: {
          type: 'query',
          query: { language_version: 3, bindings: {}, source: 'button()' },
        },
        backend_policy: { allow: [], deny: [], prefer: [] },
      },
    };

    render(
      <ActionNodeFields
        nodeId="ui-continue"
        operation={operation}
        execution={{ target_wait: { mode: 'none', timeout_ms: 0, poll_interval_ms: 0 } }}
        resourceCatalog={{
          application: [
            {
              kind: 'application',
              nodeId: 'app-before',
              nodeLabel: '打开微信',
              available: true,
            },
            {
              kind: 'application',
              nodeId: 'app-after',
              nodeLabel: '后置应用',
              available: false,
              unavailableReason: '不会在当前节点之前必定执行',
            },
          ],
          browser: [],
        }}
        onChange={onChange}
        onExecutionChange={vi.fn()}
        onOpenEditor={vi.fn()}
      />,
    );

    const resourceSelect = screen.getByRole('combobox', { name: '应用节点' });
    expect(resourceSelect).toHaveTextContent('后置应用');
    expect(screen.getByText('不会在当前节点之前必定执行')).toBeVisible();
    fireEvent.click(resourceSelect);
    expect(screen.getByRole('option', { name: /后置应用/ })).toBeDisabled();
    expect(screen.getByRole('option', { name: /打开微信/ })).toHaveTextContent('内部编号：app-before');

    fireEvent.click(screen.getByRole('option', { name: /打开微信/ }));
    expect(onChange).toHaveBeenCalledWith({
      ...operation,
      target: {
        ...operation.target,
        scope: {
          type: 'application',
          resource: { producer_node_id: 'app-before', output_name: 'session' },
        },
      },
    });
  });
});
