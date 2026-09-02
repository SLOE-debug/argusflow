import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import {
  EMPTY_WORKFLOW_RESOURCE_CATALOG,
  type UiOperation,
} from '../../../../features/workflow';
import { ActionNodeFields } from './ActionNodeFields';

describe('ActionNodeFields', () => {
  it('shows the target directly and opens the AQL document outside the main form', () => {
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
    expect(screen.getByRole('textbox', { name: '名称' })).toHaveValue('保存');
    expect(screen.getByRole('combobox', { name: '目标类型' })).toHaveTextContent('控件');
    expect(screen.queryByText('执行方式')).not.toBeInTheDocument();
    expect(screen.queryByText(/UIA|OCR|CDP/)).not.toBeInTheDocument();
    expect(screen.queryByText('尚未检查当前目标')).not.toBeInTheDocument();

    const aqlButton = screen.getByRole('button', { name: '编辑 AQL 查询' });
    expect(aqlButton).not.toHaveClass('w-full');
    fireEvent.click(aqlButton);
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

    fireEvent.change(screen.getByRole('spinbutton', { name: '最多等待目标秒数' }), {
      target: { value: '8' },
    });
    expect(onExecutionChange).toHaveBeenCalledWith({
      target_wait: { mode: 'bounded', timeout_ms: 8_000, poll_interval_ms: 100 },
    });

    expect(screen.getByText('秒')).toBeVisible();
    expect(screen.queryByText('毫秒')).not.toBeInTheDocument();
    expect(screen.queryByText('检查间隔')).not.toBeInTheDocument();
    expect(screen.getByText('超时后，节点失败并停止后续步骤。')).toBeVisible();
    fireEvent.click(screen.getByRole('checkbox', { name: '等待目标出现' }));
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
              resourceLabel: '微信',
              available: true,
            },
            {
              kind: 'application',
              nodeId: 'app-after',
              nodeLabel: '后置应用',
              resourceLabel: '后置应用',
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

    const resourceSelect = screen.getByRole('combobox', { name: '应用 / 窗口' });
    expect(resourceSelect).toHaveTextContent('后置应用');
    expect(screen.getByText('不会在当前节点之前必定执行')).toBeVisible();
    fireEvent.click(resourceSelect);
    expect(screen.getByRole('option', { name: /后置应用/ })).toBeDisabled();
    expect(screen.getByRole('option', { name: /打开微信/ })).toHaveTextContent('来源：打开微信');

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
