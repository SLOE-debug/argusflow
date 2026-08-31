import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import type { WorkflowSymbolRegistry } from '../../../features/workflow';
import { ValueField } from './ValueField';

/** ValueField 状态机测试使用的最小值目录。 */
const SYMBOLS: WorkflowSymbolRegistry = {
  inputs: [{
    id: 'input:contact_name',
    kind: 'workflow_input',
    name: 'contact_name',
    label: 'contact_name',
    valueType: 'text',
    available: true,
  }],
  variables: [],
  nodeOutputs: [],
};

describe('ValueField', () => {
  it('can switch a reference to an expression or back to a literal', () => {
    const onChange = vi.fn();
    render(
      <ValueField
        label="调试值"
        value={{
          type: 'ref',
          source: { type: 'workflow_input', key: 'contact_name' },
          pointer: '',
        }}
        symbols={SYMBOLS}
        allowExpression
        onChange={onChange}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'fx 高级表达式' }));
    expect(onChange).toHaveBeenCalledWith({ type: 'expression', source: '' });

    fireEvent.click(screen.getByRole('button', { name: '使用常量' }));
    expect(onChange).toHaveBeenCalledWith({ type: 'literal', value: '' });
  });
});
