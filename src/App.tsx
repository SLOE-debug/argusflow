import { useEffect, useState, type CSSProperties } from 'react';

import { WindowTitleBar } from './components/shell/WindowTitleBar';
import { EditorPrimaryActions } from './components/workflow/EditorPrimaryActions';
import { EditorToolbarControls } from './components/workflow/EditorToolbarControls';
import { NodeInspector } from './components/workflow/NodeInspector';
import { NodePalette } from './components/workflow/NodePalette';
import { WorkflowCanvas } from './components/workflow/WorkflowCanvas';
import { ComponentDrillDown } from './components/workflow/ComponentDrillDown';
import { WorkflowOverview } from './components/workflow/WorkflowOverview';
import { WorkflowWorkspace } from './components/workflow/WorkflowWorkspace';
import { WorkspaceStatusBar } from './components/workflow/WorkspaceStatusBar';
import { PanelResizeHandle } from './components/ui';
import { resolveWorkflowStatus } from './components/workflow/workflowStatus';
import type { StructuredEditorTarget } from './components/workflow/structuredEditorTarget';
import { useWorkspaceEditor } from './components/workflow/useWorkspaceEditor';
import { useWorkflowStudio } from './features/workflow/useWorkflowStudio';

/** 左侧节点库的默认与可调整宽度边界。 */
const LIBRARY_PANEL_WIDTH = {
  default: 248,
  min: 208,
  max: 360,
} as const;

/** 属性检查器的默认与可调整宽度边界；结构化文档不再占用此区域。 */
const INSPECTOR_PANEL_WIDTH = {
  default: 312,
  min: 272,
  max: 480,
} as const;

/** 工作台的全局主视图。 */
type AppView = 'home' | 'editor';

/** ArgusFlow 桌面 IDE 工作台入口。 */
export default function App() {
  const studio = useWorkflowStudio();
  const workspaceEditor = useWorkspaceEditor();
  const [dockOpen, setDockOpen] = useState(true);
  const [libraryWidth, setLibraryWidth] = useState<number>(LIBRARY_PANEL_WIDTH.default);
  const [inspectorWidth, setInspectorWidth] = useState<number>(INSPECTOR_PANEL_WIDTH.default);
  const [appView, setAppView] = useState<AppView>('editor');
  /** 当前通过组件节点双击进入的精确版本；null 表示主流程。 */
  const [drillDownComponentId, setDrillDownComponentId] = useState<string | null>(null);

  useEffect(() => {
    const hasConsoleContent =
      studio.events.length > 0 ||
      (studio.report !== null && !studio.report.valid) ||
      studio.errorMessage !== null;

    if (hasConsoleContent) {
      setDockOpen(true);
    }
  }, [studio.errorMessage, studio.events.length, studio.report]);

  /** 根据左右面板的当前拖拽宽度组装工作区网格。 */
  const mainGridStyle: CSSProperties = {
    gridTemplateColumns: `${libraryWidth}px minmax(0, 1fr) ${inspectorWidth}px`,
  };

  /** 从 Inspector 进入结构化编辑时同步展开统一 Workspace Dock。 */
  const openStructuredEditor = (target: StructuredEditorTarget) => {
    workspaceEditor.openEditor(target);
    setDockOpen(true);
  };
  const workflowStatus = resolveWorkflowStatus(
    studio.running,
    studio.report,
    studio.errorMessage,
  );
  const drillDownDefinition = resolveDrillDownDefinition(
    studio.nodes,
    drillDownComponentId,
  );

  return (
    <main
      data-theme="daylight"
      className={
        'grid h-full w-full grid-rows-[40px_minmax(0,1fr)_40px] ' +
        'bg-slate-50 text-slate-800'
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
        editorCommands={appView === 'editor' ? (
          <EditorToolbarControls
            store={studio.flowStore}
          />
        ) : null}
        editorActions={appView === 'editor' ? (
          <EditorPrimaryActions
            running={studio.running}
            onValidate={() => void studio.validate()}
            onRun={() => void studio.run()}
            onPublish={() => undefined}
          />
        ) : null}
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
          <div
            className="grid min-h-0"
            style={mainGridStyle}
          >
            <div className="relative min-h-0 min-w-0">
              <NodePalette
                store={studio.flowStore}
                componentCatalog={studio.componentCatalog}
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
            <WorkflowWorkspace
              dockOpen={dockOpen}
              editorState={workspaceEditor.state}
              events={studio.events}
              nodes={studio.nodes}
              report={studio.report}
              onDockOpenChange={setDockOpen}
              onDockHeightChange={workspaceEditor.setDockHeight}
              onEditorModeChange={workspaceEditor.setMode}
              onCloseEditor={workspaceEditor.closeEditor}
              onUpdateNode={studio.updateNodeById}
              canvas={(
                <>
                  <WorkflowCanvas
                    store={studio.flowStore}
                    componentCatalog={studio.componentCatalog}
                    onAddNode={studio.addNode}
                    onAddConnectedNode={studio.addConnectedNode}
                    onConnect={studio.connect}
                    onReconnect={studio.reconnect}
                    onNodeDoubleClick={(nodeId) => {
                      const node = studio.nodes.find((candidate) => candidate.id === nodeId);
                      if (node?.data.kind === 'component') setDrillDownComponentId(nodeId);
                    }}
                  />
                  {drillDownDefinition ? (
                    <ComponentDrillDown
                      definition={drillDownDefinition}
                      componentCatalog={studio.componentCatalog}
                      events={studio.events}
                      onClose={() => setDrillDownComponentId(null)}
                    />
                  ) : null}
                </>
              )}
            />
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
                componentCatalog={studio.componentCatalog}
                onNameChange={studio.setWorkflowName}
                onVariablesChange={studio.updateVariables}
                onInputDefinitionsChange={studio.updateInputDefinitions}
                onRunInputValuesChange={studio.updateRunInputValues}
                onPermissionsChange={studio.updatePermissions}
                onUpdateNode={studio.updateNode}
                onUpdateEdgeBranch={studio.updateEdgeBranch}
                onOpenStructuredEditor={openStructuredEditor}
                onDelete={studio.deleteSelection}
                onCreateComponent={studio.createComponent}
              />
            </div>
          </div>
          <WorkspaceStatusBar
            store={studio.flowStore}
            status={workflowStatus}
            libraryWidth={libraryWidth}
            inspectorWidth={inspectorWidth}
          />
        </>
      )}
    </main>
  );
}

/** 从当前组件实例的精确版本引用解析下钻定义。 */
function resolveDrillDownDefinition(
  nodes: ReadonlyArray<import('./features/workflow/workflowModel').WorkflowCanvasNode>,
  nodeId: string | null,
) {
  if (!nodeId) return null;
  const node = nodes.find((candidate) => candidate.id === nodeId);
  if (node?.data.kind !== 'component') return null;
  return node.data.componentDefinition;
}
