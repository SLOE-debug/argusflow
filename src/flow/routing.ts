import { getFlowNodeLookup } from './nodeLookup';
import { ObstacleIndex } from './obstacleIndex';
import { planEdgeRoute, type PlannedEdgeRoute } from './routePlanner';
import type { RouteResult } from './routingTypes';
import type {
  FlowEdge,
  FlowNode,
  RoutedEdge,
} from './types';

/** 路由 Facade 使用的长期节点障碍物索引。 */
export type RoutingIndex = ObstacleIndex;

/** 为批量精确路由构建一次可复用的节点空间索引。 */
export function createRoutingIndex(
  nodes: ReadonlyArray<FlowNode>,
): RoutingIndex {
  const index = new ObstacleIndex();
  index.syncAll(nodes);
  return index;
}

/** 为一条边生成精确结算路径；仅缺少端点节点时无法构造视觉路线。 */
export function routeEdge(
  edge: FlowEdge,
  nodes: ReadonlyArray<FlowNode>,
  previous?: RoutedEdge,
  sharedIndex?: RoutingIndex,
): RouteResult | null {
  return planRoute(
    edge,
    nodes,
    previous,
    sharedIndex,
    'exact',
  )?.result ?? null;
}

/** 为交互帧生成旧路修补优先、局部 OVG 兜底的可见预览路径。 */
export function previewEdgeRoute(
  edge: FlowEdge,
  nodes: ReadonlyArray<FlowNode>,
  previous?: RoutedEdge,
  sharedIndex?: RoutingIndex,
): RouteResult | null {
  return planRoute(
    edge,
    nodes,
    previous,
    sharedIndex,
    'fast',
  )?.result ?? null;
}

/** Facade 内部组合共享索引与单边规划器。 */
function planRoute(
  edge: FlowEdge,
  nodes: ReadonlyArray<FlowNode>,
  previous: RoutedEdge | undefined,
  sharedIndex: RoutingIndex | undefined,
  quality: 'fast' | 'exact',
): PlannedEdgeRoute | null {
  const obstacleIndex = sharedIndex ?? createRoutingIndex(nodes);
  return planEdgeRoute(
    edge,
    getFlowNodeLookup(nodes),
    obstacleIndex,
    previous,
    quality,
  );
}
