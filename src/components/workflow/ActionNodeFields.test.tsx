import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import type { AutomationAction } from '../../features/workflow/contracts';
import { ActionNodeFields } from './ActionNodeFields';

describe('ActionNodeFields', () => {
  it('edits the AQL source through a complete Action contract', () => {
    const onChange = vi.fn();
    const action: AutomationAction = {
      type: 'click',
      target: {
        locator: {
          type: 'query',
          query: { language_version: 1, source: 'button(name = "保存")' },
        },
        backend_preference: 'windows_uia',
      },
    };

    render(<ActionNodeFields action={action} onChange={onChange} />);

    expect(screen.getByRole('textbox', { name: 'AQL 查询' })).toHaveValue(
      'button(name = "保存")',
    );
    expect(screen.getByText('实时 AQL 分析仅在 ArgusFlow 桌面应用中可用。')).toBeVisible();

    fireEvent.change(screen.getByRole('textbox', { name: 'AQL 查询' }), {
      target: { value: 'button(name = "确定")' },
    });

    expect(onChange).toHaveBeenCalledWith({
      type: 'click',
      target: {
        locator: {
          type: 'query',
          query: { language_version: 1, source: 'button(name = "确定")' },
        },
        backend_preference: 'windows_uia',
      },
    });
  });
});
