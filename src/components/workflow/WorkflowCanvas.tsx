import {
  Background,
  BackgroundVariant,
  Controls,
  MiniMap,
  ReactFlow,
  type Connection,
  type EdgeMouseHandler,
  type NodeMouseHandler,
  type OnConnect,
  type OnEdgesChange,
  type OnNodesChange,
} from '@xyflow/react';

import type {
  WorkflowCanvasEdge,
  WorkflowCanvasNode,
} from '../../features/workflow/workflowModel';
import { WorkflowNodeCard } from './WorkflowNodeCard';

const nodeTypes = { workflow: WorkflowNodeCard };

type WorkflowCanvasProps = {
  /** 当前画布节点及其运行/校验状态。 */
  nodes: WorkflowCanvasNode[];
  /** 当前画布连接，边 ID 必须在删除和选中时保持稳定。 */
  edges: WorkflowCanvasEdge[];
  /** React Flow 节点变更处理器。 */
  onNodesChange: OnNodesChange<WorkflowCanvasNode>;
  /** React Flow 边变更处理器。 */
  onEdgesChange: OnEdgesChange<WorkflowCanvasEdge>;
  /** 新连接创建后的业务处理器。 */
  onConnect: OnConnect;
  /** 创建连接前执行的节点入度/出度及自环校验。 */
  isValidConnection: (connection: Connection | WorkflowCanvasEdge) => boolean;
  /** 选中节点或清除节点选择。 */
  onSelectNode: (nodeId: string | null) => void;
  /** 选中边或清除边选择。 */
  onSelectEdge: (edgeId: string | null) => void;
};

/** 工作流可视化编辑画布，负责 React Flow 展示和选择事件编排。 */
export function WorkflowCanvas({
  nodes,
  edges,
  onNodesChange,
  onEdgesChange,
  onConnect,
  isValidConnection,
  onSelectNode,
  onSelectEdge,
}: WorkflowCanvasProps) {
  const handleNodeClick: NodeMouseHandler<WorkflowCanvasNode> = (_event, node) => {
    onSelectNode(node.id);
    onSelectEdge(null);
  };
  const handleEdgeClick: EdgeMouseHandler<WorkflowCanvasEdge> = (_event, edge) => {
    onSelectEdge(edge.id);
    onSelectNode(null);
  };

  return (
    <div className="argusflow-grid h-full min-h-0">
      <ReactFlow
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        onConnect={onConnect}
        isValidConnection={isValidConnection}
        onNodeClick={handleNodeClick}
        onEdgeClick={handleEdgeClick}
        onPaneClick={() => {
          onSelectNode(null);
          onSelectEdge(null);
        }}
        deleteKeyCode={null}
        fitView
        fitViewOptions={{ padding: 0.2 }}
        minZoom={0.35}
        maxZoom={1.7}
        defaultEdgeOptions={{ type: 'smoothstep' }}
      >
        <Background color="#28405b" gap={28} size={1} variant={BackgroundVariant.Dots} />
        <Controls position="bottom-left" showInteractive={false} />
        <MiniMap
          position="bottom-right"
          pannable
          zoomable
          nodeColor="#1f789d"
          maskColor="rgba(5, 12, 23, 0.72)"
          className="!border !border-[#243a54] !bg-[#101f33]"
        />
      </ReactFlow>
    </div>
  );
}
