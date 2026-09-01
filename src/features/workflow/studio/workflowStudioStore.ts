import { createFlowStore } from '../../../flow';
import {
  DEFAULT_EDGES,
  DEFAULT_NODES,
  DEFAULT_ROOT_SCOPE_ID,
  DEFAULT_SCOPE_METADATA,
  DEFAULT_SELECTED_NODE_ID,
  DEFAULT_WORKFLOW_DOCUMENTS,
  DEFAULT_WORKFLOW_INPUTS,
  DEFAULT_WORKFLOW_NAME,
  DEFAULT_WORKFLOW_PERMISSIONS,
  DEFAULT_WORKFLOW_VARIABLES,
} from '../model/defaultWorkflowTemplate';
import type {
  WorkflowEdgeData,
  WorkflowNodeData,
} from '../model/workflowModel';

/** 创建带默认示例、多作用域元数据和初始选择的工作流 Store。 */
export function createWorkflowStudioStore() {
  const store = createFlowStore<WorkflowNodeData, WorkflowEdgeData>({
    metadata: {
      workflowName: DEFAULT_WORKFLOW_NAME,
      inputs: DEFAULT_WORKFLOW_INPUTS,
      variables: DEFAULT_WORKFLOW_VARIABLES,
      permissions: DEFAULT_WORKFLOW_PERMISSIONS,
      rootScopeId: DEFAULT_ROOT_SCOPE_ID,
      scopeMetadata: DEFAULT_SCOPE_METADATA,
    },
    nodes: DEFAULT_NODES,
    edges: DEFAULT_EDGES,
    activeDocumentId: DEFAULT_ROOT_SCOPE_ID,
    documents: DEFAULT_WORKFLOW_DOCUMENTS,
  });
  store.getState().selectNodes([DEFAULT_SELECTED_NODE_ID]);
  return store;
}
