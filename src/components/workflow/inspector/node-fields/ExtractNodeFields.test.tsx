import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import type { UiOperation } from '../../../../features/workflow';
import { ExtractNodeFields } from './ExtractNodeFields';

describe('ExtractNodeFields', () => {
  it('clearly labels an attribute name as a parameter of its field source', () => {
    const onChange = vi.fn();
    const operation: Extract<UiOperation, { type: 'extract' }> = {
      type: 'extract',
      target: {
        scope: { type: 'current' },
        locator: {
          type: 'query',
          query: { language_version: 1, source: 'a[href]' },
        },
        backend_policy: { allow: [], deny: [], prefer: [] },
      },
      cardinality: 'many',
      fields: [
        { name: 'title', source: { type: 'text' } },
        { name: 'url', source: { type: 'attribute', name: 'href' } },
      ],
    };

    render(<ExtractNodeFields operation={operation} onChange={onChange} />);

    expect(screen.getByText('字段名称')).toBeVisible();
    expect(screen.getByText('读取类型')).toBeVisible();
    expect(screen.getByText('元素属性名')).toBeVisible();
    expect(screen.getByRole('textbox', { name: '提取字段 2 元素属性名' }))
      .toHaveValue('href');

    fireEvent.change(
      screen.getByRole('textbox', { name: '提取字段 2 元素属性名' }),
      { target: { value: 'data-url' } },
    );

    expect(onChange).toHaveBeenCalledWith({
      ...operation,
      fields: [
        operation.fields[0],
        { name: 'url', source: { type: 'attribute', name: 'data-url' } },
      ],
    });
  });
});
