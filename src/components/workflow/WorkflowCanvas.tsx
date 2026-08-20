import type { StoreApi } from 'zustand';

import {
  FlowCanvas,
  FlowProvider,
  type FlowAnchorSide,
  type FlowPoint,
  type FlowState,
} from '../../flow';
import type {
  WorkflowEdgeData,
  WorkflowNodeData,
} from '../../features/workflow/workflowModel';
import { workflowNodeRegistry } from './WorkflowNodeCard';

type WorkflowCanvasProps = {
  store: StoreApi<FlowState<WorkflowNodeData, WorkflowEdgeData>>;
  onAddNode: (kind: WorkflowNodeData['kind'], position: FlowPoint) => void;
  onConnect: (
    source: string,
    target: string,
    sourceSide?: FlowAnchorSide,
    targetSide?: FlowAnchorSide,
  ) => boolean;
  onReconnect: (
    edgeId: string,
    endpoint: 'source' | 'target',
    nodeId: string,
    side?: FlowAnchorSide,
  ) => boolean;
};

/** 将 ArgusFlow 节点注册表和业务约束接入自研 Flow 画布。 */
export function WorkflowCanvas({
  store,
  onAddNode,
  onConnect,
  onReconnect,
}: WorkflowCanvasProps) {
  const addWorkflowNode = (kind: string, position: FlowPoint) => {
    onAddNode(kind as WorkflowNodeData['kind'], position);
  };

  return (
    <FlowProvider store={store}>
      <FlowCanvas
        registry={workflowNodeRegistry}
        onAddNode={addWorkflowNode}
        onConnect={onConnect}
        onReconnect={onReconnect}
      />
    </FlowProvider>
  );
}
