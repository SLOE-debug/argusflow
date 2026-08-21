import { useEffect, useMemo, useRef, useState } from 'react';

import { previewEdgeRoute } from './routing';
import type { FlowEdge, FlowNode, RoutedEdge } from './types';

type RouteRequest = Readonly<{
  revision: number;
  nodes: ReadonlyArray<FlowNode>;
  edges: ReadonlyArray<FlowEdge>;
}>;

type RouteResponse = Readonly<{
  revision: number;
  routes: ReadonlyArray<RoutedEdge>;
}>;

/** 使用主线程实时跟随预览与带背压的 Worker 精确路由组合。 */
export function useEdgeRoutes(
  nodes: ReadonlyArray<FlowNode>,
  edges: ReadonlyArray<FlowEdge>,
): ReadonlyArray<RoutedEdge> {
  const [exactRoutes, setExactRoutes] = useState<ReadonlyMap<string, RoutedEdge>>(
    new Map(),
  );
  /** 上一次预览所见的节点，用于识别端点位置变化。 */
  const previousNodes = useRef<ReadonlyMap<string, FlowNode>>(new Map());
  /** 最新文档请求的递增版本。 */
  const revision = useRef(0);
  /** Worker 是否正在计算一个精确请求。 */
  const workerBusy = useRef(false);
  /** 正在计算时仅保留最后一份待处理文档。 */
  const pendingRequest = useRef<RouteRequest | null>(null);
  /** 尝试发送待处理请求的稳定入口，由 Worker 生命周期 Effect 装配。 */
  const dispatchPending = useRef<() => void>(() => undefined);

  const preview = useMemo(() => {
    /** 只有位置或尺寸变化的节点才需要调整相邻边的实时预览。 */
    const changedNodeIds = new Set(nodes.flatMap((node) => {
      const previous = previousNodes.current.get(node.id);
      return !previous
        || previous.position.x !== node.position.x
        || previous.position.y !== node.position.y
        || previous.size.width !== node.size.width
        || previous.size.height !== node.size.height
        ? [node.id]
        : [];
    }));
    previousNodes.current = new Map(nodes.map((node) => [node.id, node]));

    return edges.flatMap((edge) => {
      const exact = exactRoutes.get(edge.id);
      if (
        exact
        && !changedNodeIds.has(edge.source.nodeId)
        && !changedNodeIds.has(edge.target.nodeId)
      ) {
        return [exact];
      }
      const route = previewEdgeRoute(edge, nodes, exact);
      return route ? [route] : [];
    });
  }, [edges, exactRoutes, nodes]);

  useEffect(() => {
    const currentWorker = new Worker(
      new URL('./routing.worker.ts', import.meta.url),
      { type: 'module' },
    );
    /** Worker 空闲时发送最后一份快照，保证队列不会随拖动持续增长。 */
    const sendPending = () => {
      if (workerBusy.current || !pendingRequest.current) return;
      const request = pendingRequest.current;
      pendingRequest.current = null;
      workerBusy.current = true;
      currentWorker.postMessage(request);
    };
    /** 接收精确路径；过期结果只用于释放 Worker，不更新界面。 */
    const receive = (event: MessageEvent<RouteResponse>) => {
      workerBusy.current = false;
      if (event.data.revision === revision.current) {
        setExactRoutes(new Map(
          event.data.routes.map((route) => [route.edgeId, route]),
        ));
      }
      sendPending();
    };

    dispatchPending.current = sendPending;
    currentWorker.addEventListener('message', receive);
    sendPending();
    return () => {
      currentWorker.removeEventListener('message', receive);
      currentWorker.terminate();
      workerBusy.current = false;
      pendingRequest.current = null;
      dispatchPending.current = () => undefined;
    };
  }, []);

  useEffect(() => {
    const currentRevision = ++revision.current;
    pendingRequest.current = {
      revision: currentRevision,
      nodes,
      edges,
    };
    /** 同一动画帧内连续文档更新只唤醒一次发送逻辑。 */
    const frame = requestAnimationFrame(() => dispatchPending.current());
    return () => cancelAnimationFrame(frame);
  }, [edges, nodes]);

  return preview;
}
