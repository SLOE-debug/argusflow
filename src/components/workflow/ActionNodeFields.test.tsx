import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import type { UiOperation } from '../../features/workflow/contracts';
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
          query: { language_version: 1, source: 'button(name = "保存")' },
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
        onChange={onChange}
        onOpenEditor={onOpenEditor}
      />,
    );

    expect(screen.queryByRole('textbox', { name: 'AQL 查询' })).not.toBeInTheDocument();
    expect(screen.getByText('button(name = "保存")')).toBeVisible();
    const aqlHeading = screen.getByRole('heading', { name: '查找规则' });
    const advancedSettings = screen.getByText('高级设置');
    expect(aqlHeading.compareDocumentPosition(advancedSettings)
      & Node.DOCUMENT_POSITION_FOLLOWING).not.toBe(0);
    expect(screen.getByText('运行环境评估仅在 ArgusFlow 桌面应用中可用。')).toBeVisible();

    fireEvent.click(screen.getByRole('button', { name: '编辑规则' }));
    expect(onOpenEditor).toHaveBeenCalledWith({ type: 'aql', nodeId: 'ui-save' });
    expect(onChange).not.toHaveBeenCalled();
  });
});
