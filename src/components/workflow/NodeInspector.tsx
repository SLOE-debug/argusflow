import { useEffect, useState } from 'react';

import type {
  WorkflowCanvasEdge,
  WorkflowCanvasNode,
  WorkflowNodeData,
} from '../../features/workflow/workflowModel';
import {
  EdgeInspectorFields,
  MultipleSelection,
  NodeInspectorFields,
} from './NodeInspectorFields';
import { WorkflowInspectorFields } from './WorkflowInspectorFields';

type NodeInspectorProps = Readonly<{
  /** 当前工作流名称。 */
  workflowName: string;
  /** JSON 变量草稿。 */
  variablesDraft: string;
  /** JSON 变量错误。 */
  variablesError: string | null;
  /** 当前唯一选中的节点。 */
  node: WorkflowCanvasNode | null;
  /** 当前选中的边。 */
  edge: WorkflowCanvasEdge | null;
  /** 当前选中的节点数量。 */
  selectedCount: number;
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

  useEffect(() => {
    if (props.node || props.edge || props.selectedCount > 1) setActiveTab('selection');
  }, [props.edge, props.node, props.selectedCount]);

  /** 没有选择时，节点属性页回退到工作流设置以避免空面板。 */
  const showWorkflow = activeTab === 'workflow' || (
    !props.node && !props.edge && props.selectedCount <= 1
  );

  return (
    <aside className="z-10 flex min-h-0 min-w-0 flex-col overflow-hidden border-l border-slate-200 bg-white">
      <header className="flex h-[42px] shrink-0 border-b border-slate-200 bg-slate-50 px-2">
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
        {!showWorkflow && props.selectedCount > 1 ? (
          <MultipleSelection count={props.selectedCount} />
        ) : null}
        {!showWorkflow && props.node ? (
          <NodeInspectorFields
            node={props.node}
            onUpdate={props.onUpdateNode}
            onDelete={props.onDelete}
          />
        ) : null}
        {!showWorkflow && props.edge ? (
          <EdgeInspectorFields
            edge={props.edge}
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

/** 属性面板顶端的 42px 页签。 */
function InspectorTabButton({ active, label, onClick }: InspectorTabButtonProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={
        'relative flex h-[42px] items-center px-3 text-[12px] ' +
        (active
          ? 'font-semibold text-slate-800 after:absolute after:inset-x-3 after:bottom-0 after:h-0.5 after:bg-blue-600'
          : 'text-slate-500 hover:text-slate-800')
      }
    >
      {label}
    </button>
  );
}
