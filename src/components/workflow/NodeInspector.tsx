import { useEffect, useState } from 'react';
import { useStore, type StoreApi } from 'zustand';

import type { FlowState } from '../../flow';
import type {
  WorkflowCanvasEdge,
  WorkflowCanvasNode,
  WorkflowEdgeData,
  WorkflowNodeData,
} from '../../features/workflow/workflowModel';
import {
  EdgeInspectorFields,
  MultipleSelection,
  NodeInspectorFields,
} from './NodeInspectorFields';
import { WorkflowInspectorFields } from './WorkflowInspectorFields';

type NodeInspectorProps = Readonly<{
  /** 属性面板按选择状态订阅的工作流 Store。 */
  store: StoreApi<FlowState<WorkflowNodeData, WorkflowEdgeData>>;
  /** 当前工作流名称。 */
  workflowName: string;
  /** JSON 变量草稿。 */
  variablesDraft: string;
  /** JSON 变量错误。 */
  variablesError: string | null;
  /** 修改工作流名称。 */
  onNameChange: (name: string) => void;
  /** 修改 JSON 变量。 */
  onVariablesChange: (draft: string) => void;
  /** 修改节点字段。 */
  onUpdateNode: (data: Partial<WorkflowNodeData>) => void;
  /** 修改条件分支。 */
  onUpdateEdgeBranch: (branch: 'true' | 'false') => void;
  /** 删除当前选择。 */
  onDelete: () => void;
}>;

type InspectorTab = 'workflow' | 'selection';

/** 工作流和当前选择共用的右侧属性检查器。 */
export function NodeInspector(props: NodeInspectorProps) {
  const [activeTab, setActiveTab] = useState<InspectorTab>('selection');
  const selectedCount = useStore(
    props.store,
    (state) => state.selectedNodeIds.size,
  );
  const node = useStore(props.store, (state): WorkflowCanvasNode | null => {
    if (state.selectedNodeIds.size !== 1) return null;
    const selectedNodeId = state.selectedNodeIds.values().next().value;
    return state.nodes.find((candidate) => candidate.id === selectedNodeId) ?? null;
  });
  const edge = useStore(props.store, (state): WorkflowCanvasEdge | null => (
    state.edges.find((candidate) => candidate.id === state.selectedEdgeId) ?? null
  ));

  useEffect(() => {
    if (node || edge || selectedCount > 1) setActiveTab('selection');
  }, [edge, node, selectedCount]);

  /** 没有选择时，节点属性页回退到工作流设置以避免空面板。 */
  const showWorkflow = activeTab === 'workflow' || (
    !node && !edge && selectedCount <= 1
  );

  return (
    <aside className="z-10 flex min-h-0 min-w-0 flex-col overflow-hidden border-l border-slate-200 bg-white">
      <header className="flex h-[34px] shrink-0 items-center border-b border-slate-200 bg-slate-50 px-2">
        <InspectorTabButton
          active={showWorkflow}
          label="流程设置"
          onClick={() => setActiveTab('workflow')}
        />
        <InspectorTabButton
          active={!showWorkflow}
          label="节点属性"
          onClick={() => setActiveTab('selection')}
        />
      </header>
      <div className="min-h-0 flex-1 overflow-y-auto">
        {showWorkflow ? (
          <WorkflowInspectorFields
            workflowName={props.workflowName}
            variablesDraft={props.variablesDraft}
            variablesError={props.variablesError}
            onNameChange={props.onNameChange}
            onVariablesChange={props.onVariablesChange}
          />
        ) : null}
        {!showWorkflow && selectedCount > 1 ? (
          <MultipleSelection count={selectedCount} />
        ) : null}
        {!showWorkflow && node ? (
          <NodeInspectorFields
            node={node}
            onUpdate={props.onUpdateNode}
            onDelete={props.onDelete}
          />
        ) : null}
        {!showWorkflow && edge ? (
          <EdgeInspectorFields
            edge={edge}
            onBranchChange={props.onUpdateEdgeBranch}
            onDelete={props.onDelete}
          />
        ) : null}
      </div>
    </aside>
  );
}

type InspectorTabButtonProps = Readonly<{
  active: boolean;
  label: string;
  onClick: () => void;
}>;

/** 属性面板顶端的 34px 紧凑页签。 */
function InspectorTabButton({ active, label, onClick }: InspectorTabButtonProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={
        'relative flex h-[34px] items-center px-2.5 text-[11px] leading-none ' +
        (active
          ? 'font-semibold text-slate-800 after:absolute after:inset-x-2.5 after:bottom-0 after:h-0.5 after:bg-blue-600'
          : 'text-slate-500 hover:text-slate-800')
      }
    >
      {label}
    </button>
  );
}
