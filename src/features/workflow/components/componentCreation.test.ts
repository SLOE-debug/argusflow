import { afterEach, describe, expect, it, vi } from 'vitest';

import { createComponentFromSelection } from './componentCreation';
import {
  createNode,
  type WorkflowCanvasEdge,
  type WorkflowCanvasNode,
} from '../model/workflowModel';

afterEach(() => vi.unstubAllGlobals());

describe('component creation', () => {
  it('derives value ports and folds a single-entry single-exit selection', () => {
    let nextId = 0;
    vi.stubGlobal('crypto', { randomUUID: () => `generated-${nextId += 1}` });
    const start = withId(createNode('start'), 'start');
    const selectedDebug = withId(createNode('debug'), 'selected-debug');
    if (selectedDebug.data.kind !== 'debug') throw new Error('expected debug node');
    selectedDebug.data = {
      ...selectedDebug.data,
      value: {
        type: 'ref',
        source: { type: 'variable', name: 'seed' },
        pointer: '',
      },
    };
    const selectedLog = withId(createNode('log'), 'selected-log');
    const outsideDebug = withId(createNode('debug'), 'outside-debug');
    if (outsideDebug.data.kind !== 'debug') throw new Error('expected debug node');
    outsideDebug.data = {
      ...outsideDebug.data,
      value: {
        type: 'ref',
        source: { type: 'node', node_id: selectedDebug.id },
        pointer: '/value',
      },
    };
    const end = withId(createNode('end'), 'end');
    const nodes = [start, selectedDebug, selectedLog, outsideDebug, end];
    const edges = [
      edge('start-debug', start.id, selectedDebug.id),
      edge('debug-log', selectedDebug.id, selectedLog.id),
      edge('log-outside', selectedLog.id, outsideDebug.id),
      edge('outside-end', outsideDebug.id, end.id),
    ];

    const result = createComponentFromSelection(
      nodes,
      edges,
      new Set([selectedDebug.id, selectedLog.id]),
      '处理种子值',
      '1.0.0',
    );

    expect(result.catalogItem.definition.inputs).toEqual([
      { key: 'input_1', value_type: 'text' },
    ]);
    expect(result.catalogItem.definition.outputs).toEqual([{
      name: 'output_1',
      value: {
        type: 'ref',
        source: { type: 'node', node_id: selectedDebug.id },
        pointer: '/value',
      },
    }]);
    expect(result.catalogItem.definition.graph.scopes[0]?.nodes).toContainEqual(expect.objectContaining({
      id: selectedDebug.id,
      payload: {
        value: {
          type: 'ref',
          source: { type: 'workflow_input', key: 'input_1' },
          pointer: '',
        },
      },
    }));
    const component = result.nodes.find((node) => node.id === result.componentNodeId);
    expect(component?.data).toMatchObject({
      kind: 'component',
      component: {
        component_version: '1.0.0',
        inputs: {
          input_1: {
            type: 'ref',
            source: { type: 'variable', name: 'seed' },
            pointer: '',
          },
        },
      },
    });
    const rewrittenConsumer = result.nodes.find((node) => node.id === outsideDebug.id);
    expect(rewrittenConsumer?.data).toMatchObject({
      kind: 'debug',
      value: {
        type: 'ref',
        source: { type: 'node', node_id: result.componentNodeId },
        pointer: '/output_1',
      },
    });
    expect(result.edges).toEqual(expect.arrayContaining([
      expect.objectContaining({ target: expect.objectContaining({ nodeId: result.componentNodeId }) }),
      expect.objectContaining({ source: expect.objectContaining({ nodeId: result.componentNodeId }) }),
    ]));
  });
});

function withId(node: WorkflowCanvasNode, id: string): WorkflowCanvasNode {
  return { ...node, id };
}

function edge(id: string, source: string, target: string): WorkflowCanvasEdge {
  return {
    id,
    source: { nodeId: source },
    target: { nodeId: target },
    data: { branch: null },
  };
}
