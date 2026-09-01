import type { StoreApi } from 'zustand';

import type { FlowState } from '../../../flow';
import type { ExecutionEvent } from '../model/contracts';
import {
  applyExecutionEventToNodes,
  type NodeRunState,
  type WorkflowEdgeData,
  type WorkflowNodeData,
} from '../model/workflowModel';

type WorkflowFlowState = FlowState<WorkflowNodeData, WorkflowEdgeData>;

/** 对全部作用域文档更新节点，并保持活动文档投影与文档表引用一致。 */
export function updateAllWorkflowDocuments(
  store: StoreApi<WorkflowFlowState>,
  update: (
    nodes: ReadonlyArray<import('../model/workflowModel').WorkflowCanvasNode>,
  ) => ReadonlyArray<import('../model/workflowModel').WorkflowCanvasNode>,
) {
  store.setState((state) => {
    const documents = Object.fromEntries(Object.entries(state.documents).map(
      ([scopeId, document]) => [scopeId, { ...document, nodes: update(document.nodes) }],
    ));
    return { documents, nodes: documents[state.activeDocumentId]?.nodes ?? state.nodes };
  });
}

/** 把一个 Runtime 事件按全局节点 ID 应用到任意深度的作用域文档。 */
export function applyExecutionEventToDocuments(
  store: StoreApi<WorkflowFlowState>,
  event: ExecutionEvent,
) {
  updateAllWorkflowDocuments(store, (nodes) => applyExecutionEventToNodes(nodes, event));
}

/** 一次性设置全部作用域节点的运行状态，并可同步清理校验标记。 */
export function setAllNodeRunStates(
  store: StoreApi<WorkflowFlowState>,
  runState: NodeRunState,
  clearInvalid: boolean,
) {
  updateAllWorkflowDocuments(store, (nodes) => nodes.map((node) => ({
    ...node,
    data: {
      ...node.data,
      runState,
      invalid: clearInvalid ? false : node.data.invalid,
    },
  })));
}

/** 按全局问题节点集合更新所有作用域的校验状态。 */
export function setInvalidNodeIds(
  store: StoreApi<WorkflowFlowState>,
  invalidNodeIds: ReadonlySet<string>,
) {
  updateAllWorkflowDocuments(store, (nodes) => nodes.map((node) => ({
    ...node,
    data: { ...node.data, invalid: invalidNodeIds.has(node.id) },
  })));
}
