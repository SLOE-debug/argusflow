/// <reference lib="webworker" />

import { getFlowNodeLookup } from '../selection/nodeLookup';
import { edgeRouteFingerprint } from './routeFingerprint';
import { createRoutingIndex, routeEdge } from './routing';
import type {
  ExactEdgeRouteResponse,
  ExactRouteRequest,
  ExactRouteResponse,
} from './routingWorkerProtocol';

/** Worker 每次只精修请求标记的脏边，并返回可增量合并的显式 patch。 */
self.onmessage = (event: MessageEvent<ExactRouteRequest>) => {
  const request = event.data;
  const dirtyEdgeIds = new Set(request.dirtyEdgeIds);
  const previousRoutes = new Map(request.previousRoutes.map((route) => (
    [route.edgeId, route] as const
  )));
  const nodesById = getFlowNodeLookup(request.nodes);
  const obstacleIndex = createRoutingIndex(request.nodes);
  const routes: ExactEdgeRouteResponse[] = [];
  for (const edge of request.edges) {
    if (!dirtyEdgeIds.has(edge.id)) continue;
    const source = nodesById.get(edge.source.nodeId);
    const target = nodesById.get(edge.target.nodeId);
    if (!source || !target) continue;
    const fingerprint = edgeRouteFingerprint(edge, source, target);
    const result = routeEdge(
      edge,
      request.nodes,
      previousRoutes.get(edge.id),
      obstacleIndex,
    );
    if (!result) continue;
    if (result.kind === 'degraded') {
      routes.push({
        edgeId: edge.id,
        kind: 'failed',
        fingerprint,
        reason: result.reason,
      });
      continue;
    }
    routes.push({
      edgeId: edge.id,
      kind: 'routed',
      fingerprint,
      route: result.route,
    });
  }
  const response: ExactRouteResponse = {
    revision: request.revision,
    routes,
  };
  self.postMessage(response);
};

export {};
