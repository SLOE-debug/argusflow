import { describe, expect, it } from 'vitest';

import { createRegisteredNodeData } from '../model/workflowNodeDefinitions';
import type { WorkflowCanvasNode } from '../model/workflowModel';
import {
  countWorkflowReferences,
  findExpressionReferenceLocations,
  renameWorkflowReferences,
} from './workflowValueReferences';

/** 创建值引用事务测试所需的最小画布节点。 */
function createNode(
  id: string,
  kind: Parameters<typeof createRegisteredNodeData>[0],
): WorkflowCanvasNode {
  const data = createRegisteredNodeData(kind);
  return {
    id,
    kind: data.kind,
    position: { x: 0, y: 0 },
    size: { width: 100, height: 50 },
    data,
  };
}

describe('workflow value reference transactions', () => {
  it('renames structured refs, assignment targets and output bindings together', () => {
    const debug = createNode('debug', 'debug');
    if (debug.data.kind !== 'debug') throw new Error('expected debug data');
    debug.data = {
      ...debug.data,
      value: {
        type: 'ref',
        source: { type: 'variable', name: 'old_name' },
        pointer: '/value',
      },
      outputBindings: {
        result: {
          type: 'ref',
          source: { type: 'variable', name: 'old_name' },
          pointer: '',
        },
      },
    };
    const variable = createNode('variable', 'variable');
    if (variable.data.kind !== 'variable') throw new Error('expected variable data');
    variable.data = {
      ...variable.data,
      assignments: [{
        name: 'old_name',
        value: { type: 'literal', value: 1 },
      }],
    };

    const renamed = renameWorkflowReferences([debug, variable], {
      kind: 'variable',
      oldName: 'old_name',
      newName: 'new_name',
    });

    const renamedDebug = renamed[0];
    if (renamedDebug?.data.kind !== 'debug') throw new Error('expected renamed debug data');
    expect(renamedDebug.data.value).toMatchObject({
      source: { type: 'variable', name: 'new_name' },
    });
    expect(renamedDebug.data.outputBindings.result).toMatchObject({
      source: { type: 'variable', name: 'new_name' },
    });
    const renamedVariable = renamed[1];
    if (renamedVariable?.data.kind !== 'variable') throw new Error('expected renamed variable data');
    expect(renamedVariable.data.assignments[0]?.name).toBe('new_name');
  });

  it('counts structured references so declaration deletion can be protected', () => {
    const debug = createNode('debug', 'debug');
    if (debug.data.kind !== 'debug') throw new Error('expected debug data');
    debug.data = {
      ...debug.data,
      value: {
        type: 'ref',
        source: { type: 'workflow_input', key: 'contact_name' },
        pointer: '',
      },
      outputBindings: {
        copy: {
          type: 'ref',
          source: { type: 'workflow_input', key: 'contact_name' },
          pointer: '',
        },
      },
    };

    expect(countWorkflowReferences([debug], 'workflow_input', 'contact_name')).toBe(2);
    expect(countWorkflowReferences([debug], 'workflow_input', 'message')).toBe(0);
  });

  it('detects direct input and variable access in advanced expressions without rewriting source', () => {
    const debug = createNode('expression-debug', 'debug');
    if (debug.data.kind !== 'debug') throw new Error('expected debug data');
    debug.data = {
      ...debug.data,
      label: '组合结果',
      value: {
        type: 'expression',
        source: 'input[ "contact_name" ] + vars.prefix + vars["not_prefix"]',
      },
    };

    expect(countWorkflowReferences([debug], 'workflow_input', 'contact_name')).toBe(1);
    expect(countWorkflowReferences([debug], 'variable', 'prefix')).toBe(1);
    expect(findExpressionReferenceLocations([debug], 'variable', 'prefix')).toEqual([{
      nodeId: 'expression-debug',
      nodeLabel: '组合结果',
    }]);
    expect(findExpressionReferenceLocations([debug], 'variable', 'missing')).toEqual([]);
  });
});
