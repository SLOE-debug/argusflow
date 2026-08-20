import { describe, expect, it, vi } from 'vitest';

import {
  DEFAULT_EDGES,
  DEFAULT_NODES,
  applyExecutionEventToNodes,
  createNode,
  toWorkflowDefinition,
} from './workflowModel';

/** 验证画布模型与 Rust 工作流契约之间的转换及执行状态投影。 */
describe('workflow model', () => {
  it('maps the default canvas to the Rust workflow contract', () => {
    /** 使用固定 ID 保证契约转换测试只关注字段映射，不受随机值影响。 */
    const workflow = toWorkflowDefinition(
      '6d7d7a91-4e19-42c9-b1d8-011d4cf94330',
      'Demo',
      DEFAULT_NODES,
      DEFAULT_EDGES,
    );

    expect(workflow.schema_version).toBe(1);
    expect(workflow.nodes.map((node) => node.type)).toEqual([
      'start',
      'log',
      'delay',
      'end',
    ]);
    expect(workflow.edges).toHaveLength(3);
  });

  it('creates an editable log node with a stable contract shape', () => {
    /** 固定 UUID 结果以验证节点 ID 拼接格式，同时避免依赖真实随机源。 */
    vi.stubGlobal('crypto', { randomUUID: () => 'generated-id' });
    const node = createNode('log', 2);

    expect(node.id).toBe('log-generated-id');
    expect(node.data.message).toBeTruthy();
    vi.unstubAllGlobals();
  });

  it('applies execution events to the corresponding canvas node', () => {
    const nextNodes = applyExecutionEventToNodes(DEFAULT_NODES, {
      run_id: 'run',
      workflow_id: 'workflow',
      sequence: 1,
      node_id: 'log',
      kind: 'node_started',
      message: null,
    });

    expect(nextNodes.find((node) => node.id === 'log')?.data.runState).toBe('running');
    expect(nextNodes.find((node) => node.id === 'start')?.data.runState).toBe('idle');
  });
});
