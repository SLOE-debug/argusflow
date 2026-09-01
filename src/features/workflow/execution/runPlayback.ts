import type { ExecutionEvent, WorkflowDefinition } from '../model/contracts';
import type { NodeRunState } from '../model/workflowModel';

/** 回放游标处可直接投影到流程画布和数据视图的累计状态。 */
export type RunPlaybackState = Readonly<{
  cursor: number;
  selectedEvent: ExecutionEvent | null;
  /** 能在持久化运行快照中定位的产品节点，组件内部事件会回落到组件实例。 */
  selectedFlowNodeId: string | null;
  nodeStates: ReadonlyMap<string, NodeRunState>;
  nodeExecutionCounts: ReadonlyMap<string, number>;
  activeEdgeIds: ReadonlySet<string>;
  selectedNodeSequence: number | null;
  /** 最近一次可见 UI 写动作完成的事件序号；更早的 Scene 此后不再代表当前界面。 */
  sceneInvalidatedAtSequence: number;
}>;

/** 对事件前缀做确定性归约，拖动时间线时不依赖编辑器的当前状态。 */
export function deriveRunPlayback(
  workflow: WorkflowDefinition | null,
  events: ReadonlyArray<ExecutionEvent>,
  cursor: number,
): RunPlaybackState {
  /** 限制后的游标允许空事件用 -1 表达，而不会访问越界项。 */
  const boundedCursor = events.length === 0
    ? -1
    : Math.max(0, Math.min(cursor, events.length - 1));
  /** 所有快照节点先进入 pending，随后由事件累计覆盖。 */
  const nodeStates = new Map<string, NodeRunState>(
    workflow?.graph.scopes.flatMap((scope) => (
      scope.nodes.map((node) => [node.id, 'pending'] as const)
    )) ?? [],
  );
  const nodeExecutionCounts = new Map<string, number>();
  const activeEdgeIds = new Set<string>();
  /** 每个节点最近一次 NodeStarted 的事件序号。 */
  const nodeSequences = new Map<string, number>();
  const workflowNodeIds = new Set(nodeStates.keys());
  const nodeTypeIds = new Map(workflow?.graph.scopes.flatMap((scope) => (
    scope.nodes.map((node) => [node.id, node.type_id] as const)
  )) ?? []);
  let sceneInvalidatedAtSequence = -1;

  for (const event of events.slice(0, boundedCursor + 1)) {
    const flowNodeId = resolveFlowNodeId(event, workflowNodeIds);
    if (event.kind === 'node_started' && flowNodeId) {
      nodeStates.set(flowNodeId, 'running');
      nodeSequences.set(flowNodeId, event.sequence);
      nodeExecutionCounts.set(
        flowNodeId,
        (nodeExecutionCounts.get(flowNodeId) ?? 0) + 1,
      );
    } else if (event.kind === 'node_succeeded' && flowNodeId) {
      nodeStates.set(flowNodeId, 'success');
      if (nodeTypeIds.get(flowNodeId) === 'argus.ui') {
        sceneInvalidatedAtSequence = event.sequence;
      }
    } else if (event.kind === 'node_failed' && flowNodeId) {
      nodeStates.set(flowNodeId, 'error');
    } else if (event.kind === 'edge_traversed' && event.edge_id) {
      activeEdgeIds.add(event.edge_id);
    } else if (event.kind === 'workflow_completed' || event.kind === 'workflow_failed') {
      for (const [nodeId, state] of nodeStates) {
        if (state === 'pending') nodeStates.set(nodeId, 'skipped');
        if (state === 'running') {
          nodeStates.set(nodeId, event.kind === 'workflow_completed' ? 'success' : 'error');
        }
      }
    }
  }

  const selectedEvent = boundedCursor >= 0 ? events[boundedCursor] ?? null : null;
  const selectedFlowNodeId = selectedEvent
    ? resolveFlowNodeId(selectedEvent, workflowNodeIds)
    : null;
  return {
    cursor: boundedCursor,
    selectedEvent,
    selectedFlowNodeId,
    nodeStates,
    nodeExecutionCounts,
    activeEdgeIds,
    selectedNodeSequence: selectedFlowNodeId
      ? nodeSequences.get(selectedFlowNodeId) ?? null
      : null,
    sceneInvalidatedAtSequence,
  };
}

/** 组件事件优先定位真实内部节点；运行快照未包含它时定位外层组件实例。 */
function resolveFlowNodeId(
  event: ExecutionEvent,
  workflowNodeIds: ReadonlySet<string>,
): string | null {
  if (event.expanded_node_id && workflowNodeIds.has(event.expanded_node_id)) {
    return event.expanded_node_id;
  }
  return event.node_id && workflowNodeIds.has(event.node_id) ? event.node_id : null;
}
