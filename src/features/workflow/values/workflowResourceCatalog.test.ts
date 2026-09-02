import { describe, expect, it } from 'vitest';

import type {
  WorkflowCanvasEdge,
  WorkflowCanvasNode,
  WorkflowScopeMetadataMap,
} from '../model/workflowModel';
import { createNode } from '../model/workflowModel';
import { buildWorkflowResourceCatalog } from './workflowResourceCatalog';

describe('workflow resource catalog', () => {
  it('allows only a same-scope resource that strictly dominates the consumer', () => {
    const documents = {
      root: document(
        [node('start', 'start'), node('app-before', 'application'), node('consumer', 'ui'), node('app-after', 'application')],
        [edge('start', 'app-before'), edge('app-before', 'consumer'), edge('consumer', 'app-after')],
      ),
    };

    const catalog = buildWorkflowResourceCatalog({
      documents,
      scopeMetadata: rootMetadata('start'),
      consumerScopeId: 'root',
      consumerNodeId: 'consumer',
    });

    expect(catalog.application).toMatchObject([
      { nodeId: 'app-before', available: true },
      {
        nodeId: 'app-after',
        available: false,
        unavailableReason: '不会在当前节点之前必定执行',
      },
    ]);
  });

  it('disables a producer that only runs on another condition branch', () => {
    const documents = {
      root: document(
        [node('start', 'start'), node('branch', 'condition'), node('branch-app', 'application'), node('consumer', 'ui')],
        [edge('start', 'branch'), edge('branch', 'branch-app'), edge('branch', 'consumer')],
      ),
    };

    const catalog = buildWorkflowResourceCatalog({
      documents,
      scopeMetadata: rootMetadata('start'),
      consumerScopeId: 'root',
      consumerNodeId: 'consumer',
    });

    expect(catalog.application[0]).toMatchObject({
      nodeId: 'branch-app',
      available: false,
      unavailableReason: '不会在当前节点之前必定执行',
    });
  });

  it('allows an ancestor resource that dominates the loop container', () => {
    const documents = {
      root: document(
        [node('start', 'start'), node('root-browser', 'browser'), node('loop', 'loop')],
        [edge('start', 'root-browser'), edge('root-browser', 'loop')],
      ),
      body: document(
        [node('body-entry', 'loopEntry'), node('consumer', 'ui'), node('body-continue', 'loopContinue')],
        [edge('body-entry', 'consumer'), edge('consumer', 'body-continue')],
      ),
    };
    const scopeMetadata: WorkflowScopeMetadataMap = {
      root: { parent: null, boundary: { type: 'workflow', entry_node_id: 'start' } },
      body: {
        parent: { scope_id: 'root', node_id: 'loop' },
        boundary: {
          type: 'loop',
          entry_node_id: 'body-entry',
          continue_node_id: 'body-continue',
          complete_node_id: 'body-continue',
        },
      },
    };

    const catalog = buildWorkflowResourceCatalog({
      documents,
      scopeMetadata,
      consumerScopeId: 'body',
      consumerNodeId: 'consumer',
    });

    expect(catalog.browser[0]).toMatchObject({
      nodeId: 'root-browser',
      available: true,
    });
  });

  it('keeps sibling resources visible but unavailable and handles a missing consumer', () => {
    const documents = {
      root: document(
        [node('start', 'start'), node('left-loop', 'loop'), node('right-loop', 'loop')],
        [edge('start', 'left-loop'), edge('left-loop', 'right-loop')],
      ),
      left: document([node('left-entry', 'loopEntry'), node('left-app', 'application')], [edge('left-entry', 'left-app')]),
      right: document([node('right-entry', 'loopEntry'), node('consumer', 'ui')], [edge('right-entry', 'consumer')]),
    };
    const scopeMetadata: WorkflowScopeMetadataMap = {
      root: { parent: null, boundary: { type: 'workflow', entry_node_id: 'start' } },
      left: loopMetadata('left-loop', 'left-entry'),
      right: loopMetadata('right-loop', 'right-entry'),
    };

    const siblingCatalog = buildWorkflowResourceCatalog({
      documents,
      scopeMetadata,
      consumerScopeId: 'right',
      consumerNodeId: 'consumer',
    });
    expect(siblingCatalog.application[0]).toMatchObject({
      nodeId: 'left-app',
      available: false,
      unavailableReason: '不在当前节点可用的流程范围内',
    });

    const missingCatalog = buildWorkflowResourceCatalog({
      documents,
      scopeMetadata,
      consumerScopeId: 'right',
      consumerNodeId: 'missing',
    });
    expect(missingCatalog.application[0]).toMatchObject({
      available: false,
      unavailableReason: '当前节点不存在',
    });
  });
});

/** 建立测试所需的最小强类型画布节点，并保留注册表默认语义。 */
function node(id: string, kind: Parameters<typeof createNode>[0]): WorkflowCanvasNode {
  const created = createNode(kind);
  return { ...created, id, data: { ...created.data, label: id } };
}

/** 建立只表达控制流前后关系的测试边。 */
function edge(source: string, target: string): WorkflowCanvasEdge {
  return {
    id: `${source}-${target}`,
    source: { nodeId: source },
    target: { nodeId: target },
    data: { branch: null },
  };
}

/** 把节点和边组合成一份只读作用域文档。 */
function document(nodes: WorkflowCanvasNode[], edges: WorkflowCanvasEdge[]) {
  return { nodes, edges };
}

/** 创建单根作用域元数据。 */
function rootMetadata(entryNodeId: string): WorkflowScopeMetadataMap {
  return {
    root: { parent: null, boundary: { type: 'workflow', entry_node_id: entryNodeId } },
  };
}

/** 创建测试用子循环作用域元数据。 */
function loopMetadata(parentNodeId: string, entryNodeId: string) {
  return {
    parent: { scope_id: 'root', node_id: parentNodeId },
    boundary: {
      type: 'loop' as const,
      entry_node_id: entryNodeId,
      continue_node_id: entryNodeId,
      complete_node_id: entryNodeId,
    },
  };
}
