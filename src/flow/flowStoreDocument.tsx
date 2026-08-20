import type { FlowEdge, FlowNode, FlowPoint } from './types';
import type {
  FlowClipboard,
  FlowDocumentSnapshot,
  FlowState,
} from './flowStoreTypes';

/** 一次粘贴操作生成的新节点和新连线。 */
export type PastedSubgraph<TData, TEdgeData> = Readonly<{
  nodes: FlowNode<TData>[];
  edges: FlowEdge<TEdgeData>[];
}>;

/** 深拷贝当前文档字段，隔离后续编辑对历史的影响。 */
export function cloneDocumentSnapshot<TData, TEdgeData>(
  state: Pick<FlowState<TData, TEdgeData>, 'metadata' | 'nodes' | 'edges'>,
): FlowDocumentSnapshot<TData, TEdgeData> {
  return {
    metadata: structuredClone(state.metadata),
    nodes: structuredClone(state.nodes),
    edges: structuredClone(state.edges),
  };
}

/** 复制选择中完全位于选中节点之间的子图。 */
export function copySelectedSubgraph<TData, TEdgeData>(
  state: Pick<
    FlowState<TData, TEdgeData>,
    'nodes' | 'edges' | 'selectedNodeIds'
  >,
): FlowClipboard<TData, TEdgeData> {
  const selectedIds = state.selectedNodeIds;
  return {
    nodes: state.nodes.filter((node) => selectedIds.has(node.id)),
    edges: state.edges.filter((edge) => (
      selectedIds.has(edge.source.nodeId)
      && selectedIds.has(edge.target.nodeId)
    )),
  };
}

/** 返回移动选中节点后的集合，未选中节点保持原引用。 */
export function moveSelectedNodes<TData>(
  nodes: ReadonlyArray<FlowNode<TData>>,
  selectedNodeIds: ReadonlySet<string>,
  delta: FlowPoint,
): FlowNode<TData>[] {
  return nodes.map((node) => (
    selectedNodeIds.has(node.id)
      ? {
          ...node,
          position: {
            x: node.position.x + delta.x,
            y: node.position.y + delta.y,
          },
        }
      : node
  ));
}

/** 删除当前选择，并同步删除与已删除节点关联的边。 */
export function removeSelection<TData, TEdgeData>(
  document: FlowDocumentSnapshot<TData, TEdgeData>,
  selectedNodeIds: ReadonlySet<string>,
  selectedEdgeId: string | null,
  protectedKinds: ReadonlySet<string>,
): FlowDocumentSnapshot<TData, TEdgeData> {
  const removedNodeIds = new Set(
    document.nodes
      .filter((node) => (
        selectedNodeIds.has(node.id) && !protectedKinds.has(node.kind)
      ))
      .map((node) => node.id),
  );

  return {
    ...document,
    nodes: document.nodes.filter((node) => !removedNodeIds.has(node.id)),
    edges: document.edges.filter((edge) => (
      edge.id !== selectedEdgeId
      && !removedNodeIds.has(edge.source.nodeId)
      && !removedNodeIds.has(edge.target.nodeId)
    )),
  };
}

/** 生成带新 ID 和固定偏移的粘贴子图。 */
export function createPastedSubgraph<TData, TEdgeData>(
  clipboard: FlowClipboard<TData, TEdgeData>,
  existingNodes: ReadonlyArray<FlowNode<TData>>,
  singletonKinds: ReadonlySet<string>,
): PastedSubgraph<TData, TEdgeData> {
  const existingKinds = new Set(existingNodes.map((node) => node.kind));
  const acceptedNodes = clipboard.nodes.filter((node) => (
    !singletonKinds.has(node.kind) || !existingKinds.has(node.kind)
  ));
  const idMap = new Map(acceptedNodes.map((node) => [
    node.id,
    `${node.kind}-${crypto.randomUUID()}`,
  ]));
  const nodes = acceptedNodes.flatMap((node) => {
    const pastedId = idMap.get(node.id);
    if (!pastedId) return [];

    return [{
      ...structuredClone(node),
      id: pastedId,
      position: {
        x: node.position.x + 32,
        y: node.position.y + 32,
      },
    }];
  });
  const edges = clipboard.edges
    .filter((edge) => (
      idMap.has(edge.source.nodeId) && idMap.has(edge.target.nodeId)
    ))
    .flatMap((edge) => {
      const sourceNodeId = idMap.get(edge.source.nodeId);
      const targetNodeId = idMap.get(edge.target.nodeId);
      if (!sourceNodeId || !targetNodeId) return [];

      return [{
        ...structuredClone(edge),
        id: `edge-${crypto.randomUUID()}`,
        source: { ...edge.source, nodeId: sourceNodeId },
        target: { ...edge.target, nodeId: targetNodeId },
      }];
    });

  return { nodes, edges };
}
