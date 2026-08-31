import {
  useEffect,
  useState,
  type CSSProperties,
} from 'react';

import { WindowTitleBar } from './components/shell/WindowTitleBar';
import {
  ComponentDrillDown,
  EditorPrimaryActions,
  EditorToolbarControls,
  NodeInspector,
  NodePalette,
  resolveWorkflowStatus,
  useWorkspaceEditor,
  WorkflowCanvas,
  WorkflowDataPanel,
  RunInputsDialog,
  WorkflowOverview,
  WorkflowWorkspace,
  WorkspaceStatusBar,
} from './components/workflow';
import type { StructuredEditorTarget } from './components/workflow';
import { PanelResizeHandle } from './components/ui';
import type { StartupSnapshot } from './features/startup';
import {
  useWorkflowStudio,
  type WorkflowCanvasNode,
} from './features/workflow';

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

type AppProps = Readonly<{
  /** WGC 与 OCR 的实时启动状态，供运行门控和状态栏展示。 */
  startupStatus: StartupSnapshot;
  /** 全部桌面能力是否允许提交真实工作流运行。 */
  executionEnabled: boolean;
}>;

/** ArgusFlow 桌面 IDE 工作台入口。 */
export default function App({ startupStatus, executionEnabled }: AppProps) {
  const studio = useWorkflowStudio();
  const workspaceEditor = useWorkspaceEditor();
  const [libraryOpen, setLibraryOpen] = useState(true);
  const [dockOpen, setDockOpen] = useState(true);
  const [inspectorOpen, setInspectorOpen] = useState(true);
  const [libraryWidth, setLibraryWidth] = useState<number>(LIBRARY_PANEL_WIDTH.default);
  const [inspectorWidth, setInspectorWidth] = useState<number>(INSPECTOR_PANEL_WIDTH.default);
  const [appView, setAppView] = useState<AppView>('home');
  /** 当前通过组件节点双击进入的精确版本；null 表示主流程。 */
  const [drillDownComponentId, setDrillDownComponentId] = useState<string | null>(null);
  const [workflowDataRequest, setWorkflowDataRequest] = useState(0);
  const [runInputsOpen, setRunInputsOpen] = useState(false);

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
    gridTemplateColumns: `${libraryOpen ? libraryWidth : 0}px minmax(0, 1fr) ${inspectorOpen ? inspectorWidth : 0}px`,
  };

  /** 从 Inspector 进入结构化编辑时同步展开统一 Workspace Dock。 */
  const openStructuredEditor = (target: StructuredEditorTarget) => {
    workspaceEditor.openEditor(target);
    setDockOpen(true);
  };
  /** 激活底部工作流数据页签，并保持所有数据编辑在统一面板完成。 */
  const openWorkflowData = () => {
    setWorkflowDataRequest((current) => current + 1);
    setDockOpen(true);
  };
  const openWorkflowEditor = () => {
    setAppView('editor');
  };
  const workflowData = (
    <WorkflowDataPanel
      inputs={studio.inputDefinitions}
      runInputValues={studio.runInputValues}
      variables={studio.variables}
      running={studio.running}
      nodes={studio.nodes}
      onAddInput={(key) => studio.addInput({ key, value_type: 'text' })}
      onRenameInput={(oldKey, newKey) => studio.updateInput(oldKey, {
        key: newKey,
        value_type: 'text',
      })}
      onDeleteInput={studio.deleteInput}
      onRunInputValueChange={(key, value) => studio.setRunInputValue(key, value)}
      onAddVariable={studio.addVariable}
      onUpdateVariable={studio.updateVariable}
      onDeleteVariable={studio.deleteVariable}
      inputDefinitionsDraft={studio.inputDefinitionsDraft}
      inputDefinitionsError={studio.inputDefinitionsError}
      onImportInputs={studio.importInputsFromJson}
      onExportInputs={studio.exportInputsAsJson}
      variablesDraft={studio.variablesDraft}
      variablesError={studio.variablesError}
      onReplaceVariables={studio.replaceVariablesFromJson}
    />
  );
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
        onOpenWorkflow={openWorkflowEditor}
        editorCommands={appView === 'editor' ? (
          <EditorToolbarControls
            store={studio.flowStore}
            libraryOpen={libraryOpen}
            dockOpen={dockOpen}
            inspectorOpen={inspectorOpen}
            onLibraryOpenChange={setLibraryOpen}
            onDockOpenChange={setDockOpen}
            onInspectorOpenChange={setInspectorOpen}
          />
        ) : null}
        editorActions={appView === 'editor' ? (
          <EditorPrimaryActions
            running={studio.running}
            executionEnabled={executionEnabled}
            onValidate={() => void studio.validate()}
            onRun={() => setRunInputsOpen(true)}
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
            onOpenEditor={openWorkflowEditor}
          />
          <WorkspaceStatusBar
            store={studio.flowStore}
            status={workflowStatus}
            runtimeStatus={startupStatus}
            libraryWidth={null}
            inspectorWidth={null}
          />
        </>
      ) : (
        <>
          <div
            className="grid h-full min-h-0"
            style={mainGridStyle}
          >
            {libraryOpen ? (
              <div className="relative min-h-0 min-w-0">
                <NodePalette
                  store={studio.flowStore}
                  componentCatalog={studio.componentCatalog}
                  onCollapse={() => setLibraryOpen(false)}
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
            ) : (
              <div aria-hidden="true" />
            )}
            <WorkflowWorkspace
              dockOpen={dockOpen}
              editorState={workspaceEditor.state}
              events={studio.events}
              nodes={studio.nodes}
              edges={studio.edges}
              workflowInputs={studio.inputDefinitions}
              workflowVariables={studio.variables}
              report={studio.report}
              workflowData={workflowData}
              workflowDataRequest={workflowDataRequest}
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
            {inspectorOpen ? (
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
                  permissions={studio.permissions}
                  componentCatalog={studio.componentCatalog}
                  onNameChange={studio.setWorkflowName}
                  onCollapse={() => setInspectorOpen(false)}
                  onPermissionsChange={studio.updatePermissions}
                  onOpenWorkflowData={openWorkflowData}
                  onUpdateNode={studio.updateNode}
                  onUpdateEdgeBranch={studio.updateEdgeBranch}
                  onOpenStructuredEditor={openStructuredEditor}
                  onDelete={studio.deleteSelection}
                  onCreateComponent={studio.createComponent}
                />
              </div>
            ) : (
              <div aria-hidden="true" />
            )}
          </div>
          <WorkspaceStatusBar
            store={studio.flowStore}
            status={workflowStatus}
            runtimeStatus={startupStatus}
            libraryWidth={libraryOpen ? libraryWidth : null}
            inspectorWidth={inspectorOpen ? inspectorWidth : null}
          />
          <RunInputsDialog
            open={runInputsOpen}
            inputs={studio.inputDefinitions}
            values={studio.runInputValues}
            onOpenChange={setRunInputsOpen}
            onSubmit={(values) => {
              setRunInputsOpen(false);
              void studio.run(values);
            }}
          />
        </>
      )}
    </main>
  );
}

/** 从当前组件实例的精确版本引用解析下钻定义。 */
function resolveDrillDownDefinition(
  nodes: ReadonlyArray<WorkflowCanvasNode>,
  nodeId: string | null,
) {
  if (!nodeId) return null;
  const node = nodes.find((candidate) => candidate.id === nodeId);
  if (node?.data.kind !== 'component') return null;
  return node.data.componentDefinition;
}
