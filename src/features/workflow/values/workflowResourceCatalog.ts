import type {
  WorkflowCanvasEdge,
  WorkflowCanvasNode,
  WorkflowScopeMetadataMap,
} from '../model/workflowModel';
import { buildWorkflowNodeOutputAvailabilityIndex } from './workflowSymbolAvailability';

/** 可以产生工作流逻辑会话的内置资源节点类别。 */
export type WorkflowResourceKind = 'application' | 'browser';

/** 资源节点在属性面板下拉框中的只读展示与可用性。 */
export type WorkflowResourceOption = Readonly<{
  /** 资源节点的稳定类别。 */
  kind: WorkflowResourceKind;
  /** 写入 ResourceRef 的生产节点 ID。 */
  nodeId: string;
  /** 面向用户展示的节点名称。 */
  nodeLabel: string;
  /** 当前消费节点执行前是否保证已经产生资源。 */
  available: boolean;
  /** 不可选择时展示的控制流原因。 */
  unavailableReason?: string;
}>;

/** 应用和浏览器资源节点组成的类型化只读目录。 */
export type WorkflowResourceCatalog = Readonly<Record<
  WorkflowResourceKind,
  ReadonlyArray<WorkflowResourceOption>
>>;

/** 构建资源目录所需的完整多作用域编辑快照。 */
export type BuildWorkflowResourceCatalogArgs = Readonly<{
  /** 当前工作流的全部作用域文档。 */
  documents: Readonly<Record<string, Readonly<{
    nodes: ReadonlyArray<WorkflowCanvasNode>;
    edges: ReadonlyArray<WorkflowCanvasEdge>;
  }>>>;
  /** 作用域父子关系和固定入口。 */
  scopeMetadata: WorkflowScopeMetadataMap;
  /** 当前消费节点所在作用域。 */
  consumerScopeId: string;
  /** 当前消费节点的稳定 ID。 */
  consumerNodeId: string;
}>;

/** 没有工作流上下文时使用的稳定空目录。 */
export const EMPTY_WORKFLOW_RESOURCE_CATALOG: WorkflowResourceCatalog = {
  application: [],
  browser: [],
};

type ResourceAvailability = Readonly<
  | { available: true }
  | { available: false; unavailableReason: string }
>;

/**
 * 枚举全部资源节点，并按 Runtime 的同作用域/祖先作用域支配规则标记可用性。
 *
 * 目录保留不可用节点供用户理解当前流程，但只有必定先执行的生产节点可以选择。
 */
export function buildWorkflowResourceCatalog(
  args: BuildWorkflowResourceCatalogArgs,
): WorkflowResourceCatalog {
  /** 节点 ID 到所在作用域的索引，供跨作用域可见性判断复用。 */
  const nodeScopes = new Map<string, string>();
  /** 重复内部编号无法形成稳定 ResourceRef，所有同名节点都必须禁用。 */
  const duplicateNodeIds = new Set<string>();
  for (const [scopeId, document] of Object.entries(args.documents)) {
    for (const node of document.nodes) {
      if (nodeScopes.has(node.id)) duplicateNodeIds.add(node.id);
      else nodeScopes.set(node.id, scopeId);
    }
  }
  /** 同一作用域和目标锚点共享一次图可用性计算。 */
  const availabilityIndexes = new Map<
    string,
    ReturnType<typeof buildWorkflowNodeOutputAvailabilityIndex>
  >();

  const catalog: Record<WorkflowResourceKind, WorkflowResourceOption[]> = {
    application: [],
    browser: [],
  };
  for (const [scopeId, document] of Object.entries(args.documents)) {
    for (const node of document.nodes) {
      if (node.data.kind !== 'application' && node.data.kind !== 'browser') continue;
      const availability = resolveResourceAvailability(
        node.id,
        scopeId,
        args,
        nodeScopes,
        duplicateNodeIds,
        availabilityIndexes,
      );
      catalog[node.data.kind].push({
        kind: node.data.kind,
        nodeId: node.id,
        nodeLabel: node.data.label,
        ...availability,
      });
    }
  }

  /** 可用节点排在前面；同组内保留画布和作用域的原始顺序。 */
  catalog.application.sort(compareAvailability);
  catalog.browser.sort(compareAvailability);
  return catalog;
}

/** 返回生产节点相对当前消费节点的跨作用域可用性。 */
function resolveResourceAvailability(
  producerNodeId: string,
  producerScopeId: string,
  args: BuildWorkflowResourceCatalogArgs,
  nodeScopes: ReadonlyMap<string, string>,
  duplicateNodeIds: ReadonlySet<string>,
  availabilityIndexes: Map<
    string,
    ReturnType<typeof buildWorkflowNodeOutputAvailabilityIndex>
  >,
): ResourceAvailability {
  const consumerDocument = args.documents[args.consumerScopeId];
  if (!consumerDocument?.nodes.some((node) => node.id === args.consumerNodeId)) {
    return unavailable('当前节点不存在');
  }
  if (duplicateNodeIds.has(producerNodeId) || nodeScopes.get(producerNodeId) !== producerScopeId) {
    return unavailable('节点内部编号重复');
  }

  if (producerScopeId === args.consumerScopeId) {
    return resolveDominance(
      producerNodeId,
      args.consumerNodeId,
      producerScopeId,
      args,
      availabilityIndexes,
    )
      ? { available: true }
      : unavailable('不会在当前节点之前必定执行');
  }

  const ancestorAnchor = findAncestorAnchor(
    producerScopeId,
    args.consumerScopeId,
    args.scopeMetadata,
  );
  if (!ancestorAnchor) {
    return unavailable('不在当前节点可用的流程范围内');
  }
  return resolveDominance(
    producerNodeId,
    ancestorAnchor,
    producerScopeId,
    args,
    availabilityIndexes,
  )
    ? { available: true }
    : unavailable('不会在当前节点之前必定执行');
}

/** 找到消费作用域进入指定祖先作用域时对应的父容器节点。 */
function findAncestorAnchor(
  ancestorScopeId: string,
  consumerScopeId: string,
  metadata: WorkflowScopeMetadataMap,
): string | null {
  let currentScopeId = consumerScopeId;
  const visited = new Set([currentScopeId]);
  while (currentScopeId !== ancestorScopeId) {
    const parent = metadata[currentScopeId]?.parent;
    if (!parent || visited.has(parent.scope_id)) return null;
    if (parent.scope_id === ancestorScopeId) return parent.node_id;
    currentScopeId = parent.scope_id;
    visited.add(currentScopeId);
  }
  return null;
}

/** 复用节点输出索引判断生产节点是否从固定入口可达且严格支配目标节点。 */
function resolveDominance(
  producerNodeId: string,
  targetNodeId: string,
  scopeId: string,
  args: BuildWorkflowResourceCatalogArgs,
  indexes: Map<string, ReturnType<typeof buildWorkflowNodeOutputAvailabilityIndex>>,
): boolean {
  const document = args.documents[scopeId];
  if (!document) return false;
  const entryNodeId = args.scopeMetadata[scopeId]?.boundary.entry_node_id;
  if (!entryNodeId) return false;
  const indexKey = `${scopeId}\u0000${targetNodeId}`;
  let index = indexes.get(indexKey);
  if (!index) {
    index = buildWorkflowNodeOutputAvailabilityIndex({
      consumerNodeId: targetNodeId,
      nodes: document.nodes,
      edges: document.edges,
      entryNodeId,
    });
    indexes.set(indexKey, index);
  }
  return index.get(producerNodeId)?.available ?? false;
}

/** 创建带明确控制流原因的不可用结果。 */
function unavailable(unavailableReason: string): ResourceAvailability {
  return { available: false, unavailableReason };
}

/** 稳定地把可用节点排在不可用节点之前。 */
function compareAvailability(
  left: WorkflowResourceOption,
  right: WorkflowResourceOption,
): number {
  return Number(right.available) - Number(left.available);
}
