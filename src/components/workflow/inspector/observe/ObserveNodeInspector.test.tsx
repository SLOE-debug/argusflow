import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import {
  EMPTY_WORKFLOW_RESOURCE_CATALOG,
  type WorkflowNodeData,
} from '../../../../features/workflow';
import { ObserveNodeInspector } from './ObserveNodeInspector';

describe('ObserveNodeInspector', () => {
  it('uses a flat intent layout with technical JSON outside the main panel', () => {
    const onOpenEditor = vi.fn();
    /** 最小检查节点用于验证主路径、实现细节和开发者信息都直接平铺。 */
    const data: Extract<WorkflowNodeData, { kind: 'observe' }> = {
      kind: 'observe',
      label: '检查搜索页',
      outputBindings: {},
      resultType: 'boolean',
      observation: {
        scope: { type: 'current' },
        query: {
          language_version: 3,
          source: 'exists(button(name = "保存"))',
          bindings: {},
        },
        backend_policy: { allow: [], deny: [], prefer: [] },
        policy: { mode: 'once' },
      },
    };

    const { container } = render(
      <ObserveNodeInspector
        nodeId="observe-save"
        data={data}
        position={{ x: 250, y: 120 }}
        size={{ width: 158, height: 52 }}
        resourceCatalog={EMPTY_WORKFLOW_RESOURCE_CATALOG}
        onUpdate={vi.fn()}
        onOpenStructuredEditor={onOpenEditor}
        onDelete={vi.fn()}
      />,
    );

    expect(screen.getByText('在「当前窗口」中检查界面，返回是否找到。')).toBeVisible();
    expect(screen.getByRole('combobox', { name: '返回结果' })).toHaveTextContent('是否找到');
    expect(screen.getByRole('combobox', { name: '应用 / 窗口' })).toHaveTextContent('当前窗口');
    expect(screen.queryByText('基本信息')).not.toBeInTheDocument();
    expect(screen.getByRole('combobox', { name: '检查引擎' })).toBeVisible();
    expect(screen.getByLabelText('内部编号')).toHaveTextContent('observe-save');
    expect(container.querySelector('details')).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: '编辑 AQL 查询' }));
    expect(onOpenEditor).toHaveBeenCalledWith({ type: 'aql', nodeId: 'observe-save' });

    fireEvent.click(screen.getByRole('button', { name: '查看配置' }));
    expect(screen.getByRole('dialog', { name: '原始配置' })).toBeVisible();
    expect(screen.getByLabelText('节点原始配置')).toHaveTextContent('检查搜索页');
  });
});
