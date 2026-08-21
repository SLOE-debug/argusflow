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

/** 工作流和当前选择共用的单一右侧属性检查器。 */
export function NodeInspector(props: NodeInspectorProps) {
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

  const inspectorContext = node
    ? '节点'
    : edge
      ? '连线'
      : selectedCount > 1
        ? `${selectedCount} 项`
        : '流程';

  return (
    <aside className="z-10 flex h-full min-h-0 min-w-0 flex-col overflow-hidden border-l border-slate-200 bg-white">
      <header className="flex h-[34px] shrink-0 items-center border-b border-slate-200 bg-slate-50 px-3">
        <h2 className="text-[12px] font-semibold text-slate-800">属性</h2>
        <span className="ml-auto rounded bg-slate-200/70 px-1.5 py-0.5 text-[10px] leading-none text-slate-500">
          {inspectorContext}
        </span>
      </header>
      <div className="min-h-0 flex-1 overflow-y-auto">
        {!node && !edge && selectedCount <= 1 ? (
          <WorkflowInspectorFields
            workflowName={props.workflowName}
            variablesDraft={props.variablesDraft}
            variablesError={props.variablesError}
            onNameChange={props.onNameChange}
            onVariablesChange={props.onVariablesChange}
          />
        ) : null}
        {selectedCount > 1 ? (
          <MultipleSelection count={selectedCount} />
        ) : null}
        {node ? (
          <NodeInspectorFields
            node={node}
            onUpdate={props.onUpdateNode}
            onDelete={props.onDelete}
          />
        ) : null}
        {edge ? (
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
