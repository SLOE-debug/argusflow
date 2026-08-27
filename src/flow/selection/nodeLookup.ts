import type { FlowNode } from '../types';

/** 节点数组引用到只读 ID 索引的缓存；不可变数组可安全复用索引。 */
const NODE_LOOKUPS = new WeakMap<
  ReadonlyArray<FlowNode<unknown>>,
  ReadonlyMap<string, FlowNode<unknown>>
>();

/** 返回与当前不可变节点数组绑定的 ID 索引。 */
export function getFlowNodeLookup<TData>(
  nodes: ReadonlyArray<FlowNode<TData>>,
): ReadonlyMap<string, FlowNode<TData>> {
  /** 泛型节点数组在索引中只按 ID 读取，不会写入未知业务数据。 */
  const cacheKey = nodes as ReadonlyArray<FlowNode<unknown>>;
  const cached = NODE_LOOKUPS.get(cacheKey);
  if (cached) {
    return cached as ReadonlyMap<string, FlowNode<TData>>;
  }

  /** 新数组仅构建一次索引，后续节点订阅与边渲染直接按 ID 查询。 */
  const lookup = new Map(nodes.map((node) => [node.id, node]));
  NODE_LOOKUPS.set(
    cacheKey,
    lookup as ReadonlyMap<string, FlowNode<unknown>>,
  );
  return lookup;
}

/** 从缓存索引读取单个节点。 */
export function findFlowNode<TData>(
  nodes: ReadonlyArray<FlowNode<TData>>,
  nodeId: string,
): FlowNode<TData> | undefined {
  return getFlowNodeLookup(nodes).get(nodeId);
}
