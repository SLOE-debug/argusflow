import { describe, expect, it } from 'vitest';

import type { WorkflowNodeData } from '../model/workflowModel';
import {
  readNodeValueExpr,
  updateNodeValueExpr,
} from './workflowValueExpressions';

describe('workflow value expression locations', () => {
  it('updates one variable assignment without changing its siblings', () => {
    const data: WorkflowNodeData = {
      kind: 'variable',
      label: '设置变量',
      outputBindings: {},
      assignments: [
        { name: 'first', value: { type: 'literal', value: 1 } },
        { name: 'second', value: { type: 'literal', value: 2 } },
      ],
    };

    const updated = updateNodeValueExpr(
      data,
      { type: 'variable_assignment', index: 1 },
      { type: 'expression', source: 'vars.first + 1' },
    );
    expect(readNodeValueExpr(updated, { type: 'variable_assignment', index: 0 }))
      .toEqual({ type: 'literal', value: 1 });
    expect(readNodeValueExpr(updated, { type: 'variable_assignment', index: 1 }))
      .toEqual({ type: 'expression', source: 'vars.first + 1' });
  });

  it('does not write through a stale output binding location', () => {
    const data: WorkflowNodeData = {
      kind: 'debug',
      label: '调试',
      outputBindings: {},
      value: { type: 'literal', value: null },
    };

    expect(updateNodeValueExpr(
      data,
      { type: 'output_binding', name: 'removed' },
      { type: 'expression', source: 'result' },
    )).toBe(data);
  });
});
