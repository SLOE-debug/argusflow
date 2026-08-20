import { useEffect, useState } from 'react';
import { ChevronDown, ChevronUp } from 'lucide-react';

import { ExecutionLog } from './components/workflow/ExecutionLog';
import { NodeInspector } from './components/workflow/NodeInspector';
import { NodePalette } from './components/workflow/NodePalette';
import { RunToolbar } from './components/workflow/RunToolbar';
import { WorkflowCanvas } from './components/workflow/WorkflowCanvas';
import { useWorkflowStudio } from './features/workflow/useWorkflowStudio';

/** ArgusFlow 桌面 IDE 工作台入口。 */
export default function App() {
  const studio = useWorkflowStudio();
  const [libraryOpen, setLibraryOpen] = useState(true);
  const [inspectorOpen, setInspectorOpen] = useState(true);
  const [consoleOpen, setConsoleOpen] = useState(false);

  useEffect(() => {
    const hasConsoleContent =
      studio.events.length > 0 ||
      (studio.report !== null && !studio.report.valid) ||
      studio.errorMessage !== null;

    if (hasConsoleContent) {
      setConsoleOpen(true);
    }
  }, [studio.errorMessage, studio.events.length, studio.report]);

  /** 左右面板开关对应的工作区列布局。 */
  const mainColumns = libraryOpen
    ? inspectorOpen
      ? 'grid-cols-[226px_minmax(0,1fr)_292px]'
      : 'grid-cols-[226px_minmax(0,1fr)]'
    : inspectorOpen
      ? 'grid-cols-[minmax(0,1fr)_292px]'
      : 'grid-cols-[minmax(0,1fr)]';

  const toggleLibrary = () => setLibraryOpen((value) => !value);
  const toggleInspector = () => setInspectorOpen((value) => !value);
  const toggleConsole = () => setConsoleOpen((value) => !value);

  return (
    <main
      data-theme="daylight"
      className="grid h-full w-full grid-rows-[52px_minmax(0,1fr)_auto] bg-[#edf2f8] text-[#182236]"
    >
      <RunToolbar
        running={studio.running}
        report={studio.report}
        errorMessage={studio.errorMessage}
        onValidate={() => void studio.validate()}
        onRun={() => void studio.run()}
        onToggleLibrary={toggleLibrary}
        onToggleInspector={toggleInspector}
        onToggleConsole={toggleConsole}
      />
      <div className={`grid min-h-0 ${mainColumns}`}>
        {libraryOpen && (
          <NodePalette
            nodes={studio.nodes}
            onAdd={studio.addNode}
          />
        )}
        <section className="relative min-h-0 min-w-0 overflow-hidden">
          <WorkflowCanvas
            store={studio.flowStore}
            onAddNode={studio.addNode}
            onConnect={studio.connect}
            onReconnect={studio.reconnect}
          />
        </section>
        {inspectorOpen && (
          <NodeInspector
            workflowName={studio.workflowName}
            variablesDraft={studio.variablesDraft}
            variablesError={studio.variablesError}
            node={studio.selectedNode}
            edge={studio.selectedEdge}
            selectedCount={studio.selectedNodeIds.size}
            onNameChange={studio.setWorkflowName}
            onVariablesChange={studio.updateVariables}
            onUpdateNode={studio.updateNode}
            onUpdateEdgeBranch={studio.updateEdgeBranch}
            onDelete={studio.deleteSelection}
          />
        )}
      </div>
      <section className="z-[18] border-t border-slate-300 bg-slate-50">
        <button
          type="button"
          className="flex h-[34px] w-full items-center px-3 text-left text-slate-600 hover:bg-white"
          onClick={toggleConsole}
        >
          <span className="text-xs font-extrabold tracking-[.06em]">运行与校验</span>
          <span className="ml-2.5 text-[11px] text-slate-500">
            {studio.events.length} EVENTS
          </span>
          {consoleOpen ? (
            <ChevronDown
              className="ml-auto size-5"
              aria-hidden="true"
            />
          ) : (
            <ChevronUp
              className="ml-auto size-5"
              aria-hidden="true"
            />
          )}
        </button>
        {consoleOpen && (
          <ExecutionLog
            events={studio.events}
            report={studio.report}
          />
        )}
      </section>
    </main>
  );
}
