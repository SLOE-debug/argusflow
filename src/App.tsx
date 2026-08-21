import { useEffect, useState } from 'react';

import { WindowTitleBar } from './components/shell/WindowTitleBar';
import { EditorCommandBar } from './components/workflow/EditorCommandBar';
import { NodeInspector } from './components/workflow/NodeInspector';
import { NodePalette } from './components/workflow/NodePalette';
import { WorkflowCanvas } from './components/workflow/WorkflowCanvas';
import { WorkflowWorkspace } from './components/workflow/WorkflowWorkspace';
import { WorkspaceStatusBar } from './components/workflow/WorkspaceStatusBar';
import { resolveWorkflowStatus } from './components/workflow/workflowStatus';
import { useWorkflowStudio } from './features/workflow/useWorkflowStudio';

/** ArgusFlow 桌面 IDE 工作台入口。 */
export default function App() {
  const studio = useWorkflowStudio();
  const [libraryOpen, setLibraryOpen] = useState(true);
  const [inspectorOpen, setInspectorOpen] = useState(true);
  const [consoleOpen, setConsoleOpen] = useState(true);

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
      ? 'grid-cols-[224px_minmax(0,1fr)_336px]'
      : 'grid-cols-[224px_minmax(0,1fr)]'
    : inspectorOpen
      ? 'grid-cols-[minmax(0,1fr)_336px]'
      : 'grid-cols-[minmax(0,1fr)]';

  const toggleLibrary = () => setLibraryOpen((value) => !value);
  const toggleInspector = () => setInspectorOpen((value) => !value);
  const toggleConsole = () => setConsoleOpen((value) => !value);
  const workflowStatus = resolveWorkflowStatus(
    studio.running,
    studio.report,
    studio.errorMessage,
  );

  return (
    <main
      data-theme="daylight"
      className="grid h-full w-full grid-rows-[40px_40px_minmax(0,1fr)_40px] bg-slate-50 text-slate-800"
    >
      <WindowTitleBar
        workflowName={studio.workflowName}
        running={studio.running}
        report={studio.report}
        errorMessage={studio.errorMessage}
      />
      <EditorCommandBar
        store={studio.flowStore}
        running={studio.running}
        libraryOpen={libraryOpen}
        inspectorOpen={inspectorOpen}
        consoleOpen={consoleOpen}
        onValidate={() => void studio.validate()}
        onRun={() => void studio.run()}
        onToggleLibrary={toggleLibrary}
        onToggleInspector={toggleInspector}
        onToggleConsole={toggleConsole}
      />
      <div className={`grid min-h-0 ${mainColumns}`}>
        {libraryOpen && (
          <NodePalette
            store={studio.flowStore}
          />
        )}
        <WorkflowWorkspace
          open={consoleOpen}
          events={studio.events}
          report={studio.report}
          workflowName={studio.workflowName}
          onToggle={toggleConsole}
          canvas={(
            <WorkflowCanvas
              store={studio.flowStore}
              onAddNode={studio.addNode}
              onConnect={studio.connect}
              onReconnect={studio.reconnect}
            />
          )}
        />
        {inspectorOpen && (
          <NodeInspector
            store={studio.flowStore}
            workflowName={studio.workflowName}
            variablesDraft={studio.variablesDraft}
            variablesError={studio.variablesError}
            onNameChange={studio.setWorkflowName}
            onVariablesChange={studio.updateVariables}
            onUpdateNode={studio.updateNode}
            onUpdateEdgeBranch={studio.updateEdgeBranch}
            onDelete={studio.deleteSelection}
          />
        )}
      </div>
      <WorkspaceStatusBar
        store={studio.flowStore}
        status={workflowStatus}
      />
    </main>
  );
}
