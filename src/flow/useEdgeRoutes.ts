import { useEffect, useMemo, useRef, useState } from 'react';

import { FlowRouteEngine } from './routing/routeEngine';
import type {
  ExactRouteRequest,
  ExactRouteResponse,
} from './routing/routingWorkerProtocol';
import type {
  FlowEdge,
  FlowNode,
  RoutedEdge,
  RoutingInteraction,
} from './types';

/** 使用增量主线程预览与 drag-end Worker 精确 patch 组合。 */
export function useEdgeRoutes(
  nodes: ReadonlyArray<FlowNode>,
  edges: ReadonlyArray<FlowEdge>,
  interaction: RoutingInteraction,
): ReadonlyArray<RoutedEdge> {
  /** 每个画布实例拥有独立的长期路由缓存与空间索引。 */
  const routeEngine = useRef<FlowRouteEngine | null>(null);
  routeEngine.current ??= new FlowRouteEngine();
  /** Worker 精确 patch 应用后触发 React 读取引擎中的稳定路由数组。 */
  const [exactPatchRevision, setExactPatchRevision] = useState(0);
  /** 最新已提交精修请求的递增版本。 */
  const revision = useRef(0);
  /** 上一份同步输入用于在渲染阶段立即使旧 Worker 响应过期。 */
  const versionedInput = useRef<Readonly<{
    nodes: ReadonlyArray<FlowNode> | null;
    edges: ReadonlyArray<FlowEdge> | null;
    interaction: RoutingInteraction | null;
  }>>({ nodes: null, edges: null, interaction: null });
  /** Worker 是否正在计算一个脏边批次。 */
  const workerBusy = useRef(false);
  /** 当前正在 Worker 中计算的边；新快照会把它们重新合入最新请求。 */
  const inFlightEdgeIds = useRef<ReadonlySet<string>>(new Set());
  /** 背压期间仅保留最后一份完整快照和合并后的脏边集合。 */
  const pendingRequest = useRef<ExactRouteRequest | null>(null);
  /** 尝试发送待处理请求的稳定入口，由 Worker 生命周期 Effect 装配。 */
  const dispatchPending = useRef<() => void>(() => undefined);

  if (
    versionedInput.current.nodes !== nodes
    || versionedInput.current.edges !== edges
    || !sameRoutingInteraction(
      versionedInput.current.interaction,
      interaction,
    )
  ) {
    revision.current += 1;
    versionedInput.current = { nodes, edges, interaction };
  }

  const engineOutput = useMemo(() => routeEngine.current!.update({
    nodes,
    edges,
    interaction,
  }), [edges, exactPatchRevision, interaction, nodes]);

  useEffect(() => {
    const currentWorker = new Worker(
      new URL('./routing/routing.worker.ts', import.meta.url),
      { type: 'module' },
    );
    /** Worker 空闲时发送最后一份快照，保证队列不会随编辑持续增长。 */
    const sendPending = () => {
      if (workerBusy.current || !pendingRequest.current) return;
      const request = pendingRequest.current;
      pendingRequest.current = null;
      workerBusy.current = true;
      inFlightEdgeIds.current = new Set(request.dirtyEdgeIds);
      currentWorker.postMessage(request);
    };
    /** 只合并当前版本 patch；失败边和过期 fingerprint 由引擎保留旧路径。 */
    const receive = (event: MessageEvent<ExactRouteResponse>) => {
      workerBusy.current = false;
      inFlightEdgeIds.current = new Set();
      if (
        event.data.revision === revision.current
        && routeEngine.current!.applyExactRoutes(event.data.routes)
      ) {
        setExactPatchRevision((current) => current + 1);
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
      inFlightEdgeIds.current = new Set();
      pendingRequest.current = null;
      dispatchPending.current = () => undefined;
    };
  }, []);

  useEffect(() => {
    /** 拖拽期间只运行主线程 Fast Repair / Local OVG，不创建新 Worker 请求。 */
    if (interaction.kind === 'node-drag') return;
    const settleEdgeIds = routeEngine.current!.takeSettleEdgeIds();
    if (settleEdgeIds.size === 0) return;

    /** 最新请求覆盖旧快照，但必须合并尚未结算的全部边。 */
    const dirtyEdgeIds = new Set([
      ...settleEdgeIds,
      ...inFlightEdgeIds.current,
      ...(pendingRequest.current?.dirtyEdgeIds ?? []),
    ]);
    const currentRevision = revision.current;
    pendingRequest.current = {
      revision: currentRevision,
      nodes,
      edges,
      dirtyEdgeIds: [...dirtyEdgeIds],
      previousRoutes: engineOutput.routes.filter((route) => (
        dirtyEdgeIds.has(route.edgeId)
      )),
    };
    /** 同一动画帧内连续 idle 文档更新只唤醒一次发送逻辑。 */
    const frame = requestAnimationFrame(() => dispatchPending.current());
    return () => cancelAnimationFrame(frame);
  }, [edges, engineOutput, interaction, nodes]);

  return engineOutput.routes;
}

/** interactionId 足以标识拖拽，idle 对象引用变化不应产生新文档版本。 */
function sameRoutingInteraction(
  previous: RoutingInteraction | null,
  current: RoutingInteraction,
): boolean {
  if (!previous || previous.kind !== current.kind) return false;
  return previous.kind === 'idle'
    || previous.interactionId === current.interactionId;
}
