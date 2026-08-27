import type { FlowEdge, FlowNode } from '../types';

/** 精确描述端点身份、显式方向和两端几何的路由缓存键。 */
export function edgeRouteFingerprint(
  edge: FlowEdge,
  source: FlowNode,
  target: FlowNode,
): string {
  return [
    edge.source.nodeId,
    edge.source.side ?? '',
    edge.target.nodeId,
    edge.target.side ?? '',
    source.position.x,
    source.position.y,
    source.size.width,
    source.size.height,
    target.position.x,
    target.position.y,
    target.size.width,
    target.size.height,
  ].join('\u001f');
}
