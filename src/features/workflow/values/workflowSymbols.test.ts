import { describe, expect, it } from 'vitest';

import {
  createRegisteredNodeData,
} from '../model/workflowNodeDefinitions';
import type {
  WorkflowCanvasEdge,
  WorkflowCanvasNode,
} from '../model/workflowModel';
import {
  buildWorkflowSymbolRegistry,
  symbolToValueExpr,
} from './workflowSymbols';

/** 创建不依赖 React 渲染器的最小画布节点。 */
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

/** 创建图测试使用的普通有向边。 */
function createEdge(id: string, source: string, target: string): WorkflowCanvasEdge {
  return {
    id,
    source: { nodeId: source },
    target: { nodeId: target },
    data: { branch: null },
  };
}

describe('workflow symbol registry', () => {
  it('derives inputs, variables, native outputs and custom outputs from one snapshot', () => {
    const command = createNode('command-1', 'command');
    if (command.data.kind !== 'command') throw new Error('expected command data');
    command.data = {
      ...command.data,
      label: '执行命令',
      outputBindings: {
        'file/name': { type: 'expression', source: 'result.stdout' },
      },
    };

    const registry = buildWorkflowSymbolRegistry({
      inputs: [{ key: 'contact_name', value_type: 'text' }],
      variables: { retry_count: 0 },
      nodes: [command],
      edges: [],
    });

    expect(registry.inputs).toEqual([{
      id: 'input:contact_name',
      kind: 'workflow_input',
      name: 'contact_name',
      label: 'contact_name',
      valueType: 'text',
      available: true,
    }]);
    expect(registry.variables).toEqual([{
      id: 'variable:retry_count',
      kind: 'variable',
      name: 'retry_count',
      label: 'retry_count',
      valueType: 'json',
      available: true,
    }]);
    expect(registry.nodeOutputs).toEqual(expect.arrayContaining([
      expect.objectContaining({
        id: 'node:command-1:output:stdout',
        kind: 'node_output',
        nodeId: 'command-1',
        outputName: 'stdout',
        nodeLabel: '执行命令',
        label: '执行命令 · 标准输出',
        valueType: 'text',
        available: true,
      }),
      expect.objectContaining({
        id: 'node:command-1:output:file%2Fname',
        outputName: 'file/name',
        label: '执行命令 · file/name（自定义）',
        valueType: 'json',
        available: true,
      }),
    ]));
    expect(registry.nodeOutputs).toContainEqual(expect.objectContaining({
      id: 'node:command-1:result',
      kind: 'node_result',
      nodeId: 'command-1',
      label: '执行命令 · 整个输出对象',
    }));
  });

  it('removes outputs with their node and marks branch outputs unavailable for a consumer', () => {
    const start = createNode('start', 'start');
    const producer = createNode('producer', 'command');
    const branch = createNode('branch', 'condition');
    const consumer = createNode('consumer', 'debug');
    const args = {
      inputs: [],
      variables: {},
      nodes: [start, producer, branch, consumer],
      edges: [
        createEdge('start-branch', start.id, branch.id),
        { ...createEdge('branch-producer', branch.id, producer.id), data: { branch: 'true' } },
        { ...createEdge('branch-consumer', branch.id, consumer.id), data: { branch: 'false' } },
      ],
      consumerNodeId: consumer.id,
    } as const;

    const registry = buildWorkflowSymbolRegistry(args);
    expect(registry.nodeOutputs
      .filter((symbol) => symbol.nodeId === producer.id)
      .every((symbol) => !symbol.available && symbol.unavailableReason === '并非在所有执行路径上可用'))
      .toBe(true);

    const withoutProducer = buildWorkflowSymbolRegistry({
      ...args,
      nodes: [start, branch, consumer],
    });
    expect(withoutProducer.nodeOutputs.some((symbol) => symbol.nodeId === producer.id)).toBe(false);
  });

  it('maps each symbol back to the existing ValueExpr reference shape', () => {
    const input = buildWorkflowSymbolRegistry({
      inputs: [{ key: 'contact_name', value_type: 'text' }],
      variables: { greeting: '你好' },
      nodes: [],
      edges: [],
    });

    expect(symbolToValueExpr(input.inputs[0])).toEqual({
      type: 'ref',
      source: { type: 'workflow_input', key: 'contact_name' },
      pointer: '',
    });
    expect(symbolToValueExpr(input.variables[0])).toEqual({
      type: 'ref',
      source: { type: 'variable', name: 'greeting' },
      pointer: '',
    });

    const node = createNode('command-1', 'command');
    const registry = buildWorkflowSymbolRegistry({
      inputs: [],
      variables: {},
      nodes: [node],
      edges: [],
    });
    const output = registry.nodeOutputs.find((symbol) => symbol.outputName === 'stdout');
    if (!output) throw new Error('expected stdout symbol');
    expect(symbolToValueExpr(output)).toEqual({
      type: 'ref',
      source: { type: 'node', node_id: 'command-1' },
      pointer: '/stdout',
    });
    const result = registry.nodeOutputs.find((symbol) => symbol.kind === 'node_result');
    if (!result) throw new Error('expected whole node result symbol');
    expect(symbolToValueExpr(result)).toEqual({
      type: 'ref',
      source: { type: 'node', node_id: 'command-1' },
      pointer: '',
    });
  });

  it('escapes output names when creating JSON Pointer references', () => {
    const node = createNode('command-1', 'command');
    if (node.data.kind !== 'command') throw new Error('expected command data');
    node.data = {
      ...node.data,
      outputBindings: {
        'a/b~c': { type: 'expression', source: 'result' },
      },
    };
    const registry = buildWorkflowSymbolRegistry({
      inputs: [],
      variables: {},
      nodes: [node],
      edges: [],
    });
    const output = registry.nodeOutputs.find((symbol) => symbol.outputName === 'a/b~c');
    if (!output) throw new Error('expected custom output symbol');

    expect(symbolToValueExpr(output)).toMatchObject({ pointer: '/a~1b~0c' });
  });
});
