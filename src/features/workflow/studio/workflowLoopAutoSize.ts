import type { StoreApi } from 'zustand';

import type { FlowState } from '../../../flow';
import type {
  WorkflowEdgeData,
  WorkflowNodeData,
} from '../model/workflowModel';
import { synchronizeWorkflowLoopContainerSizes } from '../model/workflowLoopLayout';

type WorkflowFlowStore = StoreApi<FlowState<WorkflowNodeData, WorkflowEdgeData>>;

/**
 * 监听多作用域文档变化，并把 While 的派生尺寸同步回所属父文档。
 *
 * 自动尺寸不创建独立撤销记录；撤销子图编辑时会根据恢复后的子图再次计算。
 */
export function bindWorkflowLoopAutoSize(store: WorkflowFlowStore): () => void {
  let synchronizing = false;
  const synchronize = () => {
    if (synchronizing) return;
    synchronizing = true;
    store.setState((state) => {
      const documents = synchronizeWorkflowLoopContainerSizes(state.documents);
      if (documents === state.documents) return state;

      const activeDocument = documents[state.activeDocumentId];
      return {
        documents,
        nodes: activeDocument?.nodes ?? state.nodes,
      };
    });
    synchronizing = false;
  };

  synchronize();
  return store.subscribe((state, previousState) => {
    if (state.documents !== previousState.documents) synchronize();
  });
}
