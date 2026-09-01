import { useEffect, useMemo } from 'react';

import {
  FlowCanvas,
  FlowProvider,
  createFlowStore,
  type FlowEdgeLabelResolver,
} from '../../../flow';
import {
  createRunSnapshotDocuments,
  findRunNodeDisplayBounds,
  type RunPlaybackState,
  type RunPresentationSnapshot,
  type WorkflowDefinition,
} from '../../../features/workflow';
import { RUN_SNAPSHOT_NODE_REGISTRY } from './RunSnapshotNodeCard';

type RunFlowStageProps = Readonly<{
  workflow: WorkflowDefinition;
  presentation: RunPresentationSnapshot;
  playback: RunPlaybackState;
}>;

const noopAdd = () => undefined;
const noopConnectedAdd = () => false;
const noopConnect = () => false;
const BRANCH_LABELS: FlowEdgeLabelResolver = (data) => {
  const branch = (data as { branch?: string | null }).branch;
  if (!branch) return null;
  return { text: branch === 'true' ? '是' : branch === 'false' ? '否' : branch, color: '#64748b' };
};

/** 在通用 FlowCanvas 上投影运行快照，不复用或修改当前编辑器文档。 */
export function RunFlowStage({ workflow, presentation, playback }: RunFlowStageProps) {
  const baseDocuments = useMemo(
    () => createRunSnapshotDocuments(workflow, presentation),
    [presentation, workflow],
  );
  const store = useMemo(() => createFlowStore({
    activeDocumentId: workflow.graph.root_scope_id,
    documents: baseDocuments,
    metadata: {},
  }), [baseDocuments, workflow.graph.root_scope_id]);
  const followBounds = useMemo(() => (
    playback.selectedFlowNodeId
      ? findRunNodeDisplayBounds(workflow, playback.selectedFlowNodeId)
      : null
  ), [playback.selectedFlowNodeId, workflow]);

  useEffect(() => {
    const documents = Object.fromEntries(Object.entries(baseDocuments).map(([scopeId, document]) => [
      scopeId,
      {
        ...document,
        nodes: document.nodes.map((node) => ({
          ...node,
          data: {
            ...node.data,
            runState: playback.nodeStates.get(node.id) ?? 'pending',
            executionCount: playback.nodeExecutionCounts.get(node.id) ?? 0,
          },
        })),
      },
    ]));
    const selectedNodeId = playback.selectedFlowNodeId;
    const activeDocument = documents[workflow.graph.root_scope_id];
    if (!activeDocument) return;
    store.setState({
      documents,
      activeDocumentId: workflow.graph.root_scope_id,
      nodes: activeDocument.nodes,
      edges: activeDocument.edges,
      selectedNodeIds: new Set(selectedNodeId ? [selectedNodeId] : []),
      selectedEdgeId: playback.selectedEvent?.edge_id ?? null,
      activeEdgeIds: Object.fromEntries(
        [...playback.activeEdgeIds].map((edgeId) => [edgeId, Number.MAX_SAFE_INTEGER]),
      ),
    });
  }, [baseDocuments, playback, store, workflow]);

  return (
    <div className="relative h-full min-h-0 overflow-hidden bg-white">
      <FlowProvider store={store}>
        <FlowCanvas
          registry={RUN_SNAPSHOT_NODE_REGISTRY}
          edgeLabelResolver={BRANCH_LABELS}
          interactionMode="readonly"
          followBounds={followBounds}
          followPadding={{ top: 72, right: 72, bottom: 56, left: 56 }}
          onAddNode={noopAdd}
          onAddConnectedNode={noopConnectedAdd}
          onConnect={noopConnect}
          onReconnect={noopConnect}
        />
      </FlowProvider>
    </div>
  );
}
