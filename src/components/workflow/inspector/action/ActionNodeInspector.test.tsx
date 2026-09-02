import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import type { WorkflowNodeData } from '../../../../features/workflow';
import { EMPTY_WORKFLOW_RESOURCE_CATALOG } from '../../../../features/workflow';
import { ActionNodeInspector } from './ActionNodeInspector';

describe('ActionNodeInspector', () => {
  it('shows intent, execution, and technical context without disclosure controls', () => {
    const data: Extract<WorkflowNodeData, { kind: 'ui' }> = {
      kind: 'ui',
      label: '选择联系人',
      outputBindings: {},
      runState: 'idle',
      operation: {
        type: 'click',
        target: {
          scope: { type: 'current' },
          locator: {
            type: 'query',
            query: {
              language_version: 3,
              source: 'text(name = "选择联系人")',
              bindings: {},
            },
          },
          backend_policy: { allow: ['ocr_small'], deny: [], prefer: ['ocr_small'] },
        },
      },
      execution: {
        target_wait: { mode: 'bounded', timeout_ms: 5_000, poll_interval_ms: 200 },
      },
    };

    const { container } = render(
      <ActionNodeInspector
        nodeId="select-contact"
        data={data}
        position={{ x: 120, y: 240 }}
        size={{ width: 158, height: 52 }}
        resourceCatalog={EMPTY_WORKFLOW_RESOURCE_CATALOG}
        onUpdate={vi.fn()}
        onOpenStructuredEditor={vi.fn()}
        onDelete={vi.fn()}
      />,
    );

    expect(screen.getByText('在「当前窗口」中找到文字「选择联系人」并单击。')).toBeVisible();
    expect(screen.getByRole('textbox', { name: '文字' })).toHaveValue('选择联系人');
    expect(screen.getByText('输出')).toBeVisible();
    expect(screen.getByText('执行方式')).toBeVisible();
    expect(screen.getByText('开发者信息')).toBeVisible();
    expect(screen.getByRole('combobox', { name: '定位引擎' })).toBeVisible();
    expect(screen.getByLabelText('内部编号')).toHaveTextContent('select-contact');
    expect(container.querySelector('details')).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: '查看配置' }));
    expect(screen.getByRole('dialog', { name: '原始配置' })).toBeVisible();
    expect(screen.getByLabelText('节点原始配置')).toHaveTextContent('选择联系人');
  });
});
