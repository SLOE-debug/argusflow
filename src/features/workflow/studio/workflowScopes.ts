import type { FlowDocument, FlowDocumentSnapshot } from '../../../flow';
import {
  createEdge,
  createNode,
  type WorkflowCanvasEdge,
  type WorkflowCanvasNode,
  type WorkflowEdgeData,
  type WorkflowNodeData,
  type WorkflowScopeMetadataMap,
} from '../model/workflowModel';
import type { FlowScopeBoundaryContract } from '../model/contracts';

/** 新 While 子作用域的固定三边界和默认安全路径。 */
export function createLoopBodyDocument(scopeId: string): Readonly<{
  document: FlowDocument<WorkflowNodeData, WorkflowEdgeData>;
  boundary: Extract<FlowScopeBoundaryContract, { type: 'loop' }>;
}> {
  const entry = withId(createNode('loopEntry', { x: 60, y: 120 }), `${scopeId}-entry`);
  const complete = withId(createNode('loopComplete', { x: 330, y: 60 }), `${scopeId}-complete`);
  const continueNode = withId(createNode('loopContinue', { x: 330, y: 190 }), `${scopeId}-continue`);
  /** 默认直接完成，确保新容器不会意外重复到耗尽；用户可显式接入 Continue。 */
  const edges = [createEdge(entry.id, complete.id, [entry, complete, continueNode], [])];
  return {
    document: { nodes: [entry, complete, continueNode], edges },
    boundary: {
      type: 'loop',
      entry_node_id: entry.id,
      continue_node_id: continueNode.id,
      complete_node_id: complete.id,
    },
  };
}

/** 沿作用域父索引迭代收集指定根的全部后代。 */
export function collectDescendantScopeIds(
  rootScopeIds: ReadonlyArray<string>,
  metadata: WorkflowScopeMetadataMap,
): string[] {
  const collected = new Set(rootScopeIds);
  const queue = [...rootScopeIds];
  while (queue.length > 0) {
    const parentScopeId = queue.shift();
    if (!parentScopeId) continue;
    for (const [scopeId, scope] of Object.entries(metadata)) {
      if (scope.parent?.scope_id === parentScopeId && !collected.has(scopeId)) {
        collected.add(scopeId);
        queue.push(scopeId);
      }
    }
  }
  return [...collected];
}

/** 把节点和它拥有的 While 子作用域作为一个全局历史事务写入。 */
export function appendWorkflowNode(
  snapshot: FlowDocumentSnapshot<WorkflowNodeData, WorkflowEdgeData>,
  node: WorkflowCanvasNode,
  edges: ReadonlyArray<WorkflowCanvasEdge> = snapshot.edges,
): FlowDocumentSnapshot<WorkflowNodeData, WorkflowEdgeData> {
  const nodes = [...snapshot.nodes, node];
  if (node.data.kind !== 'loop') return { ...snapshot, nodes, edges };

  const child = createLoopBodyDocument(node.data.bodyScopeId);
  const scopeMetadata = snapshot.metadata.scopeMetadata as WorkflowScopeMetadataMap;
  return {
    ...snapshot,
    nodes,
    edges,
    documents: {
      ...snapshot.documents,
      [node.data.bodyScopeId]: child.document,
    },
    metadata: {
      ...snapshot.metadata,
      scopeMetadata: {
        ...scopeMetadata,
        [node.data.bodyScopeId]: {
          parent: { scope_id: snapshot.activeDocumentId, node_id: node.id },
          boundary: child.boundary,
        },
      },
    },
  };
}

/** 删除当前选择及 While 后代文档，并同步清理作用域元数据。 */
export function removeSelectedWorkflowContent(
  snapshot: FlowDocumentSnapshot<WorkflowNodeData, WorkflowEdgeData>,
  selectedNodeIds: ReadonlySet<string>,
  selectedEdgeId: string | null,
): FlowDocumentSnapshot<WorkflowNodeData, WorkflowEdgeData> {
  const protectedKinds = new Set(['start', 'loopEntry', 'loopContinue', 'loopComplete']);
  const removedNodes = snapshot.nodes.filter((node) => (
    selectedNodeIds.has(node.id) && !protectedKinds.has(node.kind)
  ));
  const removedNodeIds = new Set(removedNodes.map((node) => node.id));
  const directChildScopeIds = removedNodes.flatMap((node) => (
    node.data.kind === 'loop' ? [node.data.bodyScopeId] : []
  ));
  const scopeMetadata = snapshot.metadata.scopeMetadata as WorkflowScopeMetadataMap;
  const removedScopeIds = new Set(collectDescendantScopeIds(directChildScopeIds, scopeMetadata));
  return {
    ...snapshot,
    nodes: snapshot.nodes.filter((node) => !removedNodeIds.has(node.id)),
    edges: snapshot.edges.filter((edge) => (
      edge.id !== selectedEdgeId
      && !removedNodeIds.has(edge.source.nodeId)
      && !removedNodeIds.has(edge.target.nodeId)
    )),
    documents: Object.fromEntries(Object.entries(snapshot.documents)
      .filter(([scopeId]) => !removedScopeIds.has(scopeId))),
    metadata: {
      ...snapshot.metadata,
      scopeMetadata: Object.fromEntries(Object.entries(scopeMetadata)
        .filter(([scopeId]) => !removedScopeIds.has(scopeId))),
    },
  };
}

/** 用稳定边界 ID 替换新节点的随机 ID。 */
function withId(node: WorkflowCanvasNode, id: string): WorkflowCanvasNode {
  return { ...node, id };
}

/** 默认工作流多文档表使用的具体类型别名。 */
export type WorkflowDocuments = Readonly<Record<
  string,
  FlowDocument<WorkflowNodeData, WorkflowEdgeData>
>>;

/** 避免本模块导入方退化为弱类型的边集合别名。 */
export type WorkflowScopeEdges = ReadonlyArray<WorkflowCanvasEdge>;
