import {
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
} from 'react';

import type {
  JsonObject,
  ExecutionEvent,
  ValidationReport,
  WorkflowInputDefinition,
} from '../../../features/workflow';
import type {
  WorkflowCanvasNode,
  WorkflowCanvasEdge,
  WorkflowNodeUpdater,
} from '../../../features/workflow';
import type {
  WorkspaceEditorMode,
  WorkspaceEditorState,
} from './dock/structuredEditorTarget';
import { WORKSPACE_DOCK_HEIGHT } from './dock/useWorkspaceEditor';
import { WorkspaceDockPanel } from './dock/WorkspaceDockPanel';
import { clampDockHeight } from './dock/WorkspaceDockResizeHandle';
import { WorkspaceStructuredEditor } from './dock/WorkspaceStructuredEditor';

type WorkflowWorkspaceProps = Readonly<{
  /** 已装配 Flow Provider 的画布。 */
  canvas: ReactNode;
  /** 底部 Workspace Dock 是否展开。 */
  dockOpen: boolean;
  /** 当前结构化编辑器纯界面状态。 */
  editorState: WorkspaceEditorState;
  /** 当前运行事件。 */
  events: ReadonlyArray<ExecutionEvent>;
  /** 当前工作流节点，用于结构化文档解析和日志显示。 */
  nodes: ReadonlyArray<WorkflowCanvasNode>;
  /** 当前工作流连线，用于结构化表达式的 dominance-aware 补全。 */
  edges?: ReadonlyArray<WorkflowCanvasEdge>;
  /** 当前工作流声明的输入参数。 */
  workflowInputs?: ReadonlyArray<WorkflowInputDefinition>;
  /** 当前工作流变量。 */
  workflowVariables?: JsonObject;
  /** 最近一次结构校验结果。 */
  report: ValidationReport | null;
  /** 工作流级数据面板；缺省时不显示对应页签。 */
  workflowData?: ReactNode;
  /** 外部请求激活工作流数据页签的递增序号。 */
  workflowDataRequest?: number;
  /** 展开或折叠 Workspace Dock。 */
  onDockOpenChange: (open: boolean) => void;
  /** 保存拖拽后的 Dock 高度。 */
  onDockHeightChange: (height: number) => void;
  /** 最大化或还原当前编辑器。 */
  onEditorModeChange: (mode: WorkspaceEditorMode) => void;
  /** 关闭当前结构化编辑器。 */
  onCloseEditor: () => void;
  /** 按打开目标的节点 ID 写回文档。 */
  onUpdateNode: (nodeId: string, updater: WorkflowNodeUpdater) => void;
}>;

/** 中央工作区负责 Canvas 与统一、可调尺寸 Dock 的纵向编排。 */
export function WorkflowWorkspace({
  canvas,
  dockOpen,
  editorState,
  events,
  nodes,
  edges = [],
  workflowInputs = [],
  workflowVariables = {},
  report,
  workflowData,
  workflowDataRequest,
  onDockOpenChange,
  onDockHeightChange,
  onEditorModeChange,
  onCloseEditor,
  onUpdateNode,
}: WorkflowWorkspaceProps) {
  const workspaceRef = useRef<HTMLElement>(null);
  const [workspaceHeight, setWorkspaceHeight] = useState(800);

  useEffect(() => {
    const workspace = workspaceRef.current;
    if (!workspace) return undefined;
    const updateHeight = () => {
      const measuredHeight = workspace.getBoundingClientRect().height;
      if (measuredHeight > 0) {
        setWorkspaceHeight(measuredHeight);
      }
    };
    updateHeight();
    if (typeof ResizeObserver === 'undefined') return undefined;
    const observer = new ResizeObserver(updateHeight);
    observer.observe(workspace);
    return () => observer.disconnect();
  }, []);

  /** Dock 最多占中央工作区 75%，极矮窗口下同步收窄最小值。 */
  const maxDockHeight = Math.max(1, workspaceHeight * 0.75);
  const minDockHeight = Math.min(WORKSPACE_DOCK_HEIGHT.min, maxDockHeight);
  const dockHeight = clampDockHeight(
    editorState.dockHeight,
    minDockHeight,
    maxDockHeight,
  );
  const maximized = dockOpen && editorState.mode === 'maximized';
  const workspaceStyle: CSSProperties = {
    gridTemplateRows: !dockOpen
      ? 'minmax(0, 1fr) 38px'
      : maximized
        ? '0px minmax(0, 1fr)'
        : `minmax(0, 1fr) ${dockHeight}px`,
  };
  const structuredEditor = editorState.target ? (
    <WorkspaceStructuredEditor
      target={editorState.target}
      nodes={nodes}
      edges={edges}
      report={report}
      workflowInputs={workflowInputs}
      workflowVariables={workflowVariables}
      onUpdateNode={onUpdateNode}
    />
  ) : null;

  return (
    <section
      ref={workspaceRef}
      className="grid h-full min-h-0 min-w-0 overflow-hidden"
      style={workspaceStyle}
    >
      <div className="relative h-full min-h-0 min-w-0 overflow-hidden border-b border-slate-200">
        {canvas}
      </div>
      <WorkspaceDockPanel
        open={dockOpen}
        editorTarget={editorState.target}
        editorMode={editorState.mode}
        dockHeight={dockHeight}
        minDockHeight={minDockHeight}
        maxDockHeight={maxDockHeight}
        defaultDockHeight={WORKSPACE_DOCK_HEIGHT.preferredMin}
        structuredEditor={structuredEditor}
        events={events}
        nodes={nodes}
        report={report}
        workflowData={workflowData}
        workflowDataRequest={workflowDataRequest}
        onOpenChange={onDockOpenChange}
        onDockHeightChange={onDockHeightChange}
        onEditorModeChange={onEditorModeChange}
        onCloseEditor={onCloseEditor}
      />
    </section>
  );
}
