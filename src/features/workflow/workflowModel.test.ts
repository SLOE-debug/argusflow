import { describe, expect, it, vi } from 'vitest';

import { workflowNodeRegistry } from '../../components/workflow/WorkflowNodeCard';
import {
  WORKFLOW_NODE_SIZES,
  canConnect,
  createEdge,
  createNode,
  toWorkflowDefinition,
  type EditableNodeKind,
} from './workflowModel';

describe('workflow model', () => {
  it('maps the empty canvas to the schema v2 Rust contract', () => {
    const workflow = toWorkflowDefinition('6d7d7a91-4e19-42c9-b1d8-011d4cf94330', 'Demo', { enabled: true }, [], []);
    expect(workflow.schema_version).toBe(2);
    expect(workflow.variables).toEqual({ enabled: true });
    expect(workflow.nodes).toEqual([]);
  });

  it('creates a condition node with a JSON predicate', () => {
    vi.stubGlobal('crypto', { randomUUID: () => 'generated-id' });
    const node = createNode('condition', { x: 20, y: 40 });
    expect(node.id).toBe('condition-generated-id');
    expect(node.data.operator).toBe('equal');
    expect(node.position).toEqual({ x: 20, y: 40 });
    vi.unstubAllGlobals();
  });

  it('uses one compact size contract for models and renderers', () => {
    vi.stubGlobal('crypto', { randomUUID: () => 'generated-id' });
    const kinds = Object.keys(WORKFLOW_NODE_SIZES) as EditableNodeKind[];

    for (const kind of kinds) {
      expect(createNode(kind).size).toEqual(WORKFLOW_NODE_SIZES[kind]);
      expect(workflowNodeRegistry[kind].defaultSize).toEqual(WORKFLOW_NODE_SIZES[kind]);
    }

    vi.unstubAllGlobals();
  });

  it('assigns true and false branches and rejects a third condition edge', () => {
    let counter = 0;
    vi.stubGlobal('crypto', { randomUUID: () => String(++counter) });
    const condition = createNode('condition');
    const firstTarget = createNode('log');
    const secondTarget = createNode('delay');
    const thirdTarget = createNode('end');
    const nodes = [condition, firstTarget, secondTarget, thirdTarget];
    const first = createEdge(condition.id, firstTarget.id, nodes, []);
    const second = createEdge(condition.id, secondTarget.id, nodes, [first]);
    expect([first.data.branch, second.data.branch]).toEqual(['true', 'false']);
    expect(canConnect(nodes, [first, second], condition.id, thirdTarget.id)).toBe(false);
    vi.unstubAllGlobals();
  });
});
