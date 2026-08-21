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
  /** 新建节点并完成从现有节点开始的连线。 */
  onAddConnectedNode: (
    kind: WorkflowNodeData['kind'],
    position: FlowPoint,
    sourceNodeId: string,
    sourceSide: FlowAnchorSide,
  ) => boolean;
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
  onAddConnectedNode,
  onConnect,
  onReconnect,
}: WorkflowCanvasProps) {
  const addWorkflowNode = (kind: string, position: FlowPoint) => {
    if (isWorkflowNodeKind(kind)) onAddNode(kind, position);
  };

  const addConnectedWorkflowNode = (
    kind: string,
    position: FlowPoint,
    sourceNodeId: string,
    sourceSide: FlowAnchorSide,
  ) => isWorkflowNodeKind(kind) && onAddConnectedNode(
    kind,
    position,
    sourceNodeId,
    sourceSide,
  );

  return (
    <FlowProvider store={store}>
      <FlowCanvas
        registry={workflowNodeRegistry}
        onAddNode={addWorkflowNode}
        onAddConnectedNode={addConnectedWorkflowNode}
        onConnect={onConnect}
        onReconnect={onReconnect}
      />
    </FlowProvider>
  );
}

/** 检查通用画布传入的注册键是否属于工作流领域节点。 */
function isWorkflowNodeKind(kind: string): kind is WorkflowNodeData['kind'] {
  return Object.hasOwn(workflowNodeRegistry, kind);
}
