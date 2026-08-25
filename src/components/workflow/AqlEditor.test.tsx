import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { useAqlInspection } from '../../features/workflow/useAqlInspection';
import { AqlEditor } from './AqlEditor';

vi.mock('../../features/workflow/useAqlInspection', () => ({
  useAqlInspection: vi.fn(),
}));

describe('AqlEditor', () => {
  beforeEach(() => {
    vi.mocked(useAqlInspection).mockReturnValue({
      phase: 'ready',
      message: null,
      inspection: {
        status: 'valid',
        canonical_source: 'button(enabled=true,name="保存")',
        formatted_source: 'button(\n    enabled = true,\n    name = "保存"\n)',
        portability: { type: 'portable' },
        capabilities: [
          { backend: 'windows_uia', level: 'native', estimated_cost: 'low' },
          { backend: 'browser_cdp', level: 'hybrid', estimated_cost: 'medium' },
          { backend: 'vision', level: 'unsupported', estimated_cost: 'high' },
        ],
        warnings: [{
          kind: 'unsupported_backend',
          backend: 'vision',
          message: 'Vision 无法保证该查询的完整语义',
        }],
      },
    });
  });

  it('shows explain details and writes formatted AQL back to the node', () => {
    const onChange = vi.fn();
    render(
      <AqlEditor
        query={{ language_version: 1, source: 'button(name="保存",enabled=true)' }}
        backendPreference="windows_uia"
        onChange={onChange}
      />,
    );

    expect(screen.getByText('跨后端语义')).toBeVisible();
    expect(screen.getByText('原生')).toBeVisible();
    expect(screen.getByText('Vision 无法保证该查询的完整语义')).toBeVisible();

    fireEvent.click(screen.getByRole('button', { name: '格式化' }));
    expect(onChange).toHaveBeenCalledWith({
      language_version: 1,
      source: 'button(\n    enabled = true,\n    name = "保存"\n)',
    });
  });
});
