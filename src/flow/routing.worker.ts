/// <reference lib="webworker" />

import { createRoutingIndex, routeEdge } from './routing';
import type { FlowEdge, FlowNode, RoutedEdge } from './types';

type RouteRequest = Readonly<{
  revision: number;
  nodes: ReadonlyArray<FlowNode>;
  edges: ReadonlyArray<FlowEdge>;
}>;
type RouteResponse = { revision: number; routes: RoutedEdge[] };

/** Worker 内批量计算精确路径，避免 A* 阻塞画布指针事件。 */
self.onmessage = (event: MessageEvent<RouteRequest>) => {
  const index = createRoutingIndex(event.data.nodes);
  const routes = event.data.edges.flatMap((edge) => routeEdge(edge, event.data.nodes, undefined, index) ?? []);
  const response: RouteResponse = { revision: event.data.revision, routes };
  self.postMessage(response);
};

export {};
