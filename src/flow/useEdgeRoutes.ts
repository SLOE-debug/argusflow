import { useEffect, useMemo, useRef, useState } from 'react';

import { createRoutingIndex, routeEdge } from './routing';
import type { FlowEdge, FlowNode, RoutedEdge } from './types';

type RouteResponse = { revision: number; routes: RoutedEdge[] };

/** 使用主线程局部预览与 Worker 精确路由组合，丢弃过期计算结果。 */
export function useEdgeRoutes(nodes: FlowNode[], edges: FlowEdge[]): RoutedEdge[] {
  const [exactRoutes, setExactRoutes] = useState<Map<string, RoutedEdge>>(new Map());
  const previousNodes = useRef<Map<string, FlowNode>>(new Map());
  const revision = useRef(0);
  const worker = useRef<Worker | null>(null);

  const preview = useMemo(() => {
    const index = createRoutingIndex(nodes);
    const changed = new Set(nodes.filter((node) => {
      const previous = previousNodes.current.get(node.id);
      return !previous || previous.position.x !== node.position.x || previous.position.y !== node.position.y || previous.size.width !== node.size.width || previous.size.height !== node.size.height;
    }).map((node) => node.id));
    previousNodes.current = new Map(nodes.map((node) => [node.id, node]));
    return edges.flatMap((edge) => {
      const previous = exactRoutes.get(edge.id);
      if (previous && !changed.has(edge.source.nodeId) && !changed.has(edge.target.nodeId)) return [previous];
      return routeEdge(edge, nodes, previous, index) ?? [];
    });
  }, [edges, exactRoutes, nodes]);

  useEffect(() => {
    worker.current ??= new Worker(new URL('./routing.worker.ts', import.meta.url), { type: 'module' });
    const currentRevision = ++revision.current;
    const currentWorker = worker.current;
    const receive = (event: MessageEvent<RouteResponse>) => {
      if (event.data.revision !== revision.current) return;
      setExactRoutes(new Map(event.data.routes.map((route) => [route.edgeId, route])));
    };
    currentWorker.addEventListener('message', receive);
    // 同一动画帧内的连续拖拽更新只投递最后一份文档快照。
    const frame = requestAnimationFrame(() => currentWorker.postMessage({ revision: currentRevision, nodes, edges }));
    return () => { cancelAnimationFrame(frame); currentWorker.removeEventListener('message', receive); };
  }, [edges, nodes]);

  useEffect(() => () => worker.current?.terminate(), []);
  return preview;
}
