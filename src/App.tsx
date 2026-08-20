import { ExecutionLog } from './components/workflow/ExecutionLog';
import { NodeInspector } from './components/workflow/NodeInspector';
import { NodePalette } from './components/workflow/NodePalette';
import { RunToolbar } from './components/workflow/RunToolbar';
import { WorkflowCanvas } from './components/workflow/WorkflowCanvas';
import { useWorkflowStudio } from './features/workflow/useWorkflowStudio';

/** ArgusFlow 工作流编辑器的页面级布局与子区域编排入口。 */
export default function App() {
  const studio = useWorkflowStudio();

  return (
    <main className="grid h-full grid-rows-[auto_minmax(0,1fr)_auto] bg-[#08111f] text-slate-200">
      <RunToolbar
        workflowName={studio.workflowName}
        running={studio.running}
        runId={studio.runId}
        report={studio.report}
        errorMessage={studio.errorMessage}
        onNameChange={studio.setWorkflowName}
        onValidate={() => void studio.validate()}
        onRun={() => void studio.run()}
      />
      <div className="grid min-h-0 grid-cols-[190px_minmax(0,1fr)_250px]">
        <NodePalette onAdd={studio.addNode} />
        <WorkflowCanvas
          nodes={studio.nodes}
          edges={studio.edges}
          onNodesChange={studio.onNodesChange}
          onEdgesChange={studio.onEdgesChange}
          onConnect={studio.connect}
          isValidConnection={studio.isValidConnection}
          onSelectNode={studio.setSelectedNodeId}
          onSelectEdge={studio.setSelectedEdgeId}
        />
        <NodeInspector
          node={studio.selectedNode}
          selectedEdgeId={studio.selectedEdgeId}
          canDelete={studio.canDelete}
          onUpdate={studio.updateNode}
          onDelete={studio.deleteSelection}
        />
      </div>
      <ExecutionLog events={studio.events} report={studio.report} />
    </main>
  );
}
