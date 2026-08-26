import type { ObstacleChange } from './obstacleIndex';
import type { RouteSegmentIndex } from './routeSegmentIndex';
import { inflateRect, unionRects } from './routingGeometry';
import type { FlowEdge } from './types';

/** 覆盖路径沿障碍安全区外侧一像素绕行的失效查询余量。 */
const ROUTE_SWEEP_MARGIN = 2;

/** 节点到相邻边 ID 的只读失效索引。 */
export type EdgeAdjacency = ReadonlyMap<string, ReadonlySet<string>>;

/** 仅在边文档变化时重建节点邻接表。 */
export function createEdgeAdjacency(
  edges: ReadonlyArray<FlowEdge>,
): EdgeAdjacency {
  const mutable = new Map<string, Set<string>>();
  for (const edge of edges) {
    addAdjacentEdge(mutable, edge.source.nodeId, edge.id);
    addAdjacentEdge(mutable, edge.target.nodeId, edge.id);
  }
  return mutable;
}

/**
 * 合并端点邻接边与移动障碍物 swept 区域扫到的已有路线。
 *
 * 这保证未连接到移动节点、但被其挡住的边也会在下一帧重新路由。
 */
export function collectDirtyEdgeIds(
  changes: ReadonlyArray<ObstacleChange>,
  adjacency: EdgeAdjacency,
  routeSegments: RouteSegmentIndex,
): ReadonlySet<string> {
  const dirtyEdgeIds = new Set<string>();
  for (const change of changes) {
    for (const edgeId of adjacency.get(change.nodeId) ?? []) {
      dirtyEdgeIds.add(edgeId);
    }
    const sweptRect = change.previousRect && change.currentRect
      ? unionRects(change.previousRect, change.currentRect)
      : change.previousRect ?? change.currentRect;
    if (!sweptRect) continue;
    for (const edgeId of routeSegments.queryEdgeIds(
      inflateRect(sweptRect, ROUTE_SWEEP_MARGIN),
    )) {
      dirtyEdgeIds.add(edgeId);
    }
  }
  return dirtyEdgeIds;
}

/** 向节点邻接桶加入一条边。 */
function addAdjacentEdge(
  adjacency: Map<string, Set<string>>,
  nodeId: string,
  edgeId: string,
): void {
  const edgeIds = adjacency.get(nodeId) ?? new Set<string>();
  edgeIds.add(edgeId);
  adjacency.set(nodeId, edgeIds);
}
