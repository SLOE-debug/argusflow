import { useEffect, useState, type CSSProperties } from 'react';

import { WindowTitleBar } from './components/shell/WindowTitleBar';
import { EditorCommandBar } from './components/workflow/EditorCommandBar';
import { NodeInspector } from './components/workflow/NodeInspector';
import { NodePalette } from './components/workflow/NodePalette';
import { WorkflowCanvas } from './components/workflow/WorkflowCanvas';
import { WorkflowOverview } from './components/workflow/WorkflowOverview';
import { WorkflowWorkspace } from './components/workflow/WorkflowWorkspace';
import { WorkspaceStatusBar } from './components/workflow/WorkspaceStatusBar';
import { PanelResizeHandle } from './components/ui';
import { resolveWorkflowStatus } from './components/workflow/workflowStatus';
import { useWorkflowStudio } from './features/workflow/useWorkflowStudio';

/** 左侧节点库的默认与可调整宽度边界。 */
const LIBRARY_PANEL_WIDTH = {
  default: 248,
  min: 208,
  max: 360,
} as const;

/** Action/AQL 编辑器需要更宽的属性面板，同时仍允许用户按需收窄。 */
const INSPECTOR_PANEL_WIDTH = {
  default: 340,
  min: 280,
  max: 520,
} as const;

/** 工作台的全局主视图。 */
type AppView = 'home' | 'editor';

/** ArgusFlow 桌面 IDE 工作台入口。 */
export default function App() {
  const studio = useWorkflowStudio();
  const [libraryOpen, setLibraryOpen] = useState(true);
  const [inspectorOpen, setInspectorOpen] = useState(true);
  const [consoleOpen, setConsoleOpen] = useState(true);
  const [libraryWidth, setLibraryWidth] = useState<number>(LIBRARY_PANEL_WIDTH.default);
  const [inspectorWidth, setInspectorWidth] = useState<number>(INSPECTOR_PANEL_WIDTH.default);
  const [appView, setAppView] = useState<AppView>('editor');

  useEffect(() => {
    const hasConsoleContent =
      studio.events.length > 0 ||
      (studio.report !== null && !studio.report.valid) ||
      studio.errorMessage !== null;

    if (hasConsoleContent) {
      setConsoleOpen(true);
    }
  }, [studio.errorMessage, studio.events.length, studio.report]);

  /** 根据面板开关与当前拖拽宽度组装工作区网格。 */
  const mainGridStyle: CSSProperties = {
    gridTemplateColumns: [
      libraryOpen ? `${libraryWidth}px` : null,
      'minmax(0, 1fr)',
      inspectorOpen ? `${inspectorWidth}px` : null,
    ].filter((column): column is string => column !== null).join(' '),
  };

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
      className={
        'grid h-full w-full bg-slate-50 text-slate-800 ' +
        (appView === 'home'
          ? 'grid-rows-[40px_minmax(0,1fr)_40px]'
          : 'grid-rows-[40px_40px_minmax(0,1fr)_40px]')
      }
    >
      <WindowTitleBar
        workflowName={studio.workflowName}
        running={studio.running}
        report={studio.report}
        errorMessage={studio.errorMessage}
        homeActive={appView === 'home'}
        onOpenHome={() => setAppView('home')}
        onOpenWorkflow={() => setAppView('editor')}
      />
      {appView === 'home' ? (
        <>
          <WorkflowOverview
            workflowName={studio.workflowName}
            events={studio.events}
            report={studio.report}
            onOpenEditor={() => setAppView('editor')}
          />
          <WorkspaceStatusBar
            store={studio.flowStore}
            status={workflowStatus}
            libraryWidth={null}
            inspectorWidth={null}
          />
        </>
      ) : (
        <>
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
          <div
            className="grid min-h-0"
            style={mainGridStyle}
          >
            {libraryOpen && (
              <div className="relative min-h-0 min-w-0">
                <NodePalette
                  store={studio.flowStore}
                  onResetWidth={() => setLibraryWidth(LIBRARY_PANEL_WIDTH.default)}
                />
                <PanelResizeHandle
                  side="left"
                  width={libraryWidth}
                  minWidth={LIBRARY_PANEL_WIDTH.min}
                  maxWidth={LIBRARY_PANEL_WIDTH.max}
                  defaultWidth={LIBRARY_PANEL_WIDTH.default}
                  onWidthChange={setLibraryWidth}
                />
              </div>
            )}
            <WorkflowWorkspace
              open={consoleOpen}
              events={studio.events}
              report={studio.report}
              onToggle={toggleConsole}
              canvas={(
                <WorkflowCanvas
                  store={studio.flowStore}
                  onAddNode={studio.addNode}
                  onAddConnectedNode={studio.addConnectedNode}
                  onConnect={studio.connect}
                  onReconnect={studio.reconnect}
                />
              )}
            />
            {inspectorOpen && (
              <div className="relative min-h-0 min-w-0">
                <PanelResizeHandle
                  side="right"
                  width={inspectorWidth}
                  minWidth={INSPECTOR_PANEL_WIDTH.min}
                  maxWidth={INSPECTOR_PANEL_WIDTH.max}
                  defaultWidth={INSPECTOR_PANEL_WIDTH.default}
                  onWidthChange={setInspectorWidth}
                />
                <NodeInspector
                  store={studio.flowStore}
                  workflowName={studio.workflowName}
                  variablesDraft={studio.variablesDraft}
                  variablesError={studio.variablesError}
                  inputDefinitionsDraft={studio.inputDefinitionsDraft}
                  inputDefinitionsError={studio.inputDefinitionsError}
                  runInputValuesDraft={studio.runInputValuesDraft}
                  runInputValuesError={studio.runInputValuesError}
                  permissions={studio.permissions}
                  onNameChange={studio.setWorkflowName}
                  onVariablesChange={studio.updateVariables}
                  onInputDefinitionsChange={studio.updateInputDefinitions}
                  onRunInputValuesChange={studio.updateRunInputValues}
                  onPermissionsChange={studio.updatePermissions}
                  onUpdateNode={studio.updateNode}
                  onUpdateEdgeBranch={studio.updateEdgeBranch}
                  onDelete={studio.deleteSelection}
                />
              </div>
            )}
          </div>
          <WorkspaceStatusBar
            store={studio.flowStore}
            status={workflowStatus}
            libraryWidth={libraryOpen ? libraryWidth : null}
            inspectorWidth={inspectorOpen ? inspectorWidth : null}
          />
        </>
      )}
    </main>
  );
}
