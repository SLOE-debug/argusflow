import { rectsIntersect } from './geometry';
import { indexedObstacle, type ObstacleIndex } from './obstacleIndex';
import { findLocalOrthogonalRoute } from './orthogonalVisibilityGraph';
import { compactRouteCore } from './routeCompaction';
import {
  isRouteCoreClear,
  isRoutePointBlocked,
  isRoutingPortTunnelClear,
  type RouteCollisionContext,
} from './routeCollision';
import { repairRouteCore, orthogonalConnectors } from './routeRepair';
import {
  createRoutedEdge,
  orthogonalRoutePreferenceCost,
} from './routingGeometry';
import {
  buildRoutingPort,
  joinRoutingPorts,
  nodeRect,
  routingPortCandidates,
} from './routingPort';
import type { FlowEdge, FlowNode, FlowPoint, RoutedEdge } from './types';
import type { RouteQuality, RouteResult, RouterStats } from './routingTypes';

/** 自动端口切换需要抵消的路径收益，减少小幅移动时换边跳动。 */
const ROUTE_SIDE_CHANGE_PENALTY = 36;

/** 单条边规划结果以及用于累计开发态指标的局部统计。 */
export type PlannedEdgeRoute = Readonly<{
  /** 始终显式区分正常或降级的路由结果。 */
  result: RouteResult;
  /** 该边规划过程中产生的局部性能统计。 */
  stats: Pick<
    RouterStats,
    | 'nearbyObstacleCount'
    | 'fastRepairHits'
    | 'localGraphVertices'
    | 'expandedStates'
  >;
}>;

/**
 * 为一条边生成满足端口硬约束的始终可见路线。
 *
 * fast 模式依次尝试旧路修补、简单正交连接和局部 OVG；exact 模式跳过
 * 旧路修补，以当前障碍物快照重新结算。全部候选失败时仍返回 emergency 路线。
 */
export function planEdgeRoute(
  edge: FlowEdge,
  nodesById: ReadonlyMap<string, FlowNode>,
  obstacleIndex: ObstacleIndex,
  previous: RoutedEdge | undefined,
  quality: Exclude<RouteQuality, 'emergency'>,
): PlannedEdgeRoute | null {
  const sourceNode = nodesById.get(edge.source.nodeId);
  const targetNode = nodesById.get(edge.target.nodeId);
  if (!sourceNode || !targetNode) return null;

  const candidates = routingPortCandidates(
    edge,
    sourceNode,
    targetNode,
    previous,
  );
  /** 所有可行端口组合中按长度与端口稳定性综合选择最终路线。 */
  let bestRoute: RoutedEdge | null = null;
  let bestScore = Number.POSITIVE_INFINITY;
  let nearbyObstacleCount = 0;
  let bestUsedFastRepair = false;
  let localGraphVertices = 0;
  let expandedStates = 0;

  for (const candidate of candidates) {
    const sourcePort = buildRoutingPort(sourceNode, candidate.sourceSide);
    const targetPort = buildRoutingPort(targetNode, candidate.targetSide);
    const collision = createCollisionContext(
      obstacleIndex,
      sourceNode,
      targetNode,
    );
    if (
      !isRoutingPortTunnelClear(
        sourcePort,
        collision,
      )
      || !isRoutingPortTunnelClear(
        targetPort,
        collision,
      )
    ) continue;
    let corePoints: ReadonlyArray<FlowPoint> | null = null;
    let usedFastRepair = false;

    if (quality === 'fast' && previous) {
      corePoints = repairRouteCore(
        previous,
        sourcePort,
        targetPort,
        collision,
      );
      if (corePoints) usedFastRepair = true;
    }
    if (!corePoints) {
      corePoints = findSimpleCore(
        sourcePort.escape,
        targetPort.escape,
        collision,
      );
    }
    if (!corePoints) {
      const visibilityRoute = findLocalOrthogonalRoute(
        sourcePort.escape,
        targetPort.escape,
        previous,
        nodeRect(sourceNode),
        nodeRect(targetNode),
        obstacleIndex,
        collision,
      );
      if (visibilityRoute) {
        corePoints = visibilityRoute.points;
        nearbyObstacleCount = Math.max(
          nearbyObstacleCount,
          visibilityRoute.nearbyObstacleCount,
        );
        localGraphVertices += visibilityRoute.vertexCount;
        expandedStates += visibilityRoute.expandedStates;
      }
    }
    if (!corePoints) continue;
    corePoints = compactRouteCore(corePoints, collision);

    const route = createRoutedEdge(
      edge.id,
      joinRoutingPorts(sourcePort, targetPort, corePoints),
      candidate.sourceSide,
      candidate.targetSide,
      true,
    );
    const changedSide = previous
      && (
        previous.sourceSide !== candidate.sourceSide
        || previous.targetSide !== candidate.targetSide
      );
    const score = orthogonalRoutePreferenceCost(route.points)
      + (changedSide ? ROUTE_SIDE_CHANGE_PENALTY : 0);
    if (score >= bestScore) continue;
    bestRoute = route;
    bestScore = score;
    bestUsedFastRepair = usedFastRepair;

    /** 两端 side 都被锁定时不存在更优端口组合，可立即结束。 */
    if (edge.source.side && edge.target.side) break;
  }

  if (bestRoute) {
    return {
      result: { kind: 'routed', route: bestRoute, quality },
      stats: {
        nearbyObstacleCount,
        fastRepairHits: bestUsedFastRepair ? 1 : 0,
        localGraphVertices,
        expandedStates,
      },
    };
  }

  const fallback = candidates[0];
  const sourcePort = buildRoutingPort(sourceNode, fallback.sourceSide);
  const targetPort = buildRoutingPort(targetNode, fallback.targetSide);
  const collision = createCollisionContext(
    obstacleIndex,
    sourceNode,
    targetNode,
  );
  const portTunnelBlocked = !isRoutingPortTunnelClear(
    sourcePort,
    collision,
  ) || !isRoutingPortTunnelClear(
    targetPort,
    collision,
  );
  const reason = rectsIntersect(nodeRect(sourceNode), nodeRect(targetNode))
    ? 'overlapping_nodes'
    : portTunnelBlocked
      || isRoutePointBlocked(sourcePort.escape, collision)
      || isRoutePointBlocked(targetPort.escape, collision)
      ? 'blocked_port'
      : 'search_budget_exceeded';
  /** emergency 仍保留端口直线段和正交主体，只放宽主体避障保证 UI 可见。 */
  const emergencyCore = orthogonalConnectors(
    sourcePort.escape,
    targetPort.escape,
  )[0];
  const emergencyRoute = createRoutedEdge(
    edge.id,
    joinRoutingPorts(sourcePort, targetPort, emergencyCore),
    fallback.sourceSide,
    fallback.targetSide,
    true,
  );
  return {
    result: {
      kind: 'degraded',
      route: emergencyRoute,
      quality: 'emergency',
      reason,
    },
    stats: {
      nearbyObstacleCount,
      fastRepairHits: 0,
      localGraphVertices,
      expandedStates,
    },
  };
}

/** 构造仅允许端口 tunnel 进入自身端点安全区的碰撞上下文。 */
function createCollisionContext(
  obstacles: ObstacleIndex,
  source: FlowNode,
  target: FlowNode,
): RouteCollisionContext {
  return {
    obstacles,
    endpointNodeIds: new Set([source.id, target.id]),
    endpointKeepOutRects: [
      indexedObstacle(source).rect,
      indexedObstacle(target).rect,
    ],
  };
}

/** 从直线或两种 L 形候选中选择最短无碰撞主体。 */
function findSimpleCore(
  start: FlowPoint,
  end: FlowPoint,
  collision: RouteCollisionContext,
): ReadonlyArray<FlowPoint> | null {
  const candidates = orthogonalConnectors(start, end)
    .filter((points) => isRouteCoreClear(points, collision));
  if (candidates.length === 0) return null;
  return candidates.reduce((best, candidate) => (
    orthogonalRoutePreferenceCost(candidate)
      < orthogonalRoutePreferenceCost(best)
      ? candidate
      : best
  ));
}
