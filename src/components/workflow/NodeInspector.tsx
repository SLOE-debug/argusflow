import { useStore, type StoreApi } from 'zustand';

import type { FlowState } from '../../flow';
import type { FlowComponentCatalogItem } from '../../features/workflow/componentCatalog';
import type {
  WorkflowCanvasEdge,
  WorkflowCanvasNode,
  WorkflowEdgeData,
  WorkflowNodeData,
  WorkflowNodeUpdater,
} from '../../features/workflow/workflowModel';
import type { WorkflowPermissions } from '../../features/workflow/contracts';
import type {
  JsonObject,
  WorkflowInputDefinition,
} from '../../features/workflow/contracts';
import {
  EdgeInspectorFields,
  MultipleSelection,
  NodeInspectorFields,
} from './NodeInspectorFields';
import { WorkflowInspectorFields } from './WorkflowInspectorFields';
import type { StructuredEditorTarget } from './structuredEditorTarget';
import { ValueExprEditorProvider } from './ValueExprFields';

type NodeInspectorProps = Readonly<{
  /** 属性面板按选择状态订阅的工作流 Store。 */
  store: StoreApi<FlowState<WorkflowNodeData, WorkflowEdgeData>>;
  /** 当前工作流名称。 */
  workflowName: string;
  /** JSON 变量草稿。 */
  variablesDraft: string;
  /** JSON 变量错误。 */
  variablesError: string | null;
  /** 运行输入声明草稿。 */
  inputDefinitionsDraft: string;
  /** 运行输入声明错误。 */
  inputDefinitionsError: string | null;
  /** 本次运行输入草稿。 */
  runInputValuesDraft: string;
  /** 本次运行输入错误。 */
  runInputValuesError: string | null;
  /** 工作流系统能力声明。 */
  permissions: WorkflowPermissions;
  /** 当前工作区可解析的全部精确组件版本。 */
  componentCatalog?: ReadonlyArray<FlowComponentCatalogItem>;
  /** 修改工作流名称。 */
  onNameChange: (name: string) => void;
  /** 修改 JSON 变量。 */
  onVariablesChange: (draft: string) => void;
  /** 修改运行输入声明。 */
  onInputDefinitionsChange: (draft: string) => void;
  /** 修改本次运行输入。 */
  onRunInputValuesChange: (draft: string) => void;
  /** 修改工作流系统能力声明。 */
  onPermissionsChange: (permissions: WorkflowPermissions) => void;
  /** 修改节点字段。 */
  onUpdateNode: (updater: WorkflowNodeUpdater) => void;
  /** 修改条件分支。 */
  onUpdateEdgeBranch: (branch: 'true' | 'false') => void;
  /** 请求中央工作区打开结构化文档。 */
  onOpenStructuredEditor: (target: StructuredEditorTarget) => void;
  /** 删除当前选择。 */
  onDelete: () => void;
  /** 把当前多节点选择原地折叠为版本锁定组件。 */
  onCreateComponent?: (name: string, version: string) => boolean;
}>;

/** 通用 Flow Store 尚未装配工作流 metadata 时使用的稳定空值。 */
const EMPTY_WORKFLOW_INPUTS: ReadonlyArray<WorkflowInputDefinition> = [];

/** 空画布或独立组件测试中使用的稳定初始变量对象。 */
const EMPTY_WORKFLOW_VARIABLES: JsonObject = {};

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
  const nodes = useStore(props.store, (state) => state.nodes);
  const edges = useStore(props.store, (state) => state.edges);
  const workflowInputs = useStore(
    props.store,
    (state) => (
      state.metadata.inputs as WorkflowInputDefinition[] | undefined
    ) ?? EMPTY_WORKFLOW_INPUTS,
  );
  const variables = useStore(
    props.store,
    (state) => (
      state.metadata.variables as JsonObject | undefined
    ) ?? EMPTY_WORKFLOW_VARIABLES,
  );

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
            inputDefinitionsDraft={props.inputDefinitionsDraft}
            inputDefinitionsError={props.inputDefinitionsError}
            runInputValuesDraft={props.runInputValuesDraft}
            runInputValuesError={props.runInputValuesError}
            permissions={props.permissions}
            onNameChange={props.onNameChange}
            onVariablesChange={props.onVariablesChange}
            onInputDefinitionsChange={props.onInputDefinitionsChange}
            onRunInputValuesChange={props.onRunInputValuesChange}
            onPermissionsChange={props.onPermissionsChange}
          />
        ) : null}
        {selectedCount > 1 ? (
          <MultipleSelection
            count={selectedCount}
            onCreateComponent={props.onCreateComponent ?? (() => false)}
          />
        ) : null}
        {node ? (
          <ValueExprEditorProvider
            value={{
              upstreamNodes: resolveDominatingNodes(node.id, nodes, edges),
              workflowInputs,
              variableNames: Object.keys(variables).sort(),
              onOpenExpression: (location) => props.onOpenStructuredEditor({
                type: 'expression',
                nodeId: node.id,
                location,
              }),
            }}
          >
            <NodeInspectorFields
              node={node}
              componentCatalog={props.componentCatalog ?? []}
              onUpdate={props.onUpdateNode}
              onOpenStructuredEditor={props.onOpenStructuredEditor}
              onDelete={props.onDelete}
            />
          </ValueExprEditorProvider>
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

/** 计算严格支配当前节点的上游节点，避免选择只存在于部分分支的输出。 */
function resolveDominatingNodes(
  nodeId: string,
  nodes: ReadonlyArray<WorkflowCanvasNode>,
  edges: ReadonlyArray<WorkflowCanvasEdge>,
): WorkflowCanvasNode[] {
  const nodeIds = new Set(nodes.map((node) => node.id));
  const predecessors = new Map<string, string[]>();
  for (const edge of edges) {
    const current = predecessors.get(edge.target.nodeId) ?? [];
    predecessors.set(edge.target.nodeId, [...current, edge.source.nodeId]);
  }
  /** 无入边节点作为当前不完整画布的入口，避免编辑中间态产生虚假支配关系。 */
  const entryIds = new Set(nodes
    .filter((node) => (predecessors.get(node.id)?.length ?? 0) === 0)
    .map((node) => node.id));
  const dominators = new Map(nodes.map((node) => [
    node.id,
    entryIds.has(node.id) ? new Set([node.id]) : new Set(nodeIds),
  ]));
  let changed = true;
  while (changed) {
    changed = false;
    for (const node of nodes) {
      if (entryIds.has(node.id)) continue;
      const nodePredecessors = predecessors.get(node.id) ?? [];
      const next = nodePredecessors
        .map((predecessor) => dominators.get(predecessor) ?? new Set<string>())
        .reduce<Set<string>>((intersection, candidate) => new Set(
          [...intersection].filter((id) => candidate.has(id)),
        ), new Set(nodeIds));
      next.add(node.id);
      const current = dominators.get(node.id) ?? new Set<string>();
      if (current.size !== next.size || [...current].some((id) => !next.has(id))) {
        dominators.set(node.id, next);
        changed = true;
      }
    }
  }
  const currentDominators = dominators.get(nodeId) ?? new Set<string>();
  return nodes.filter((candidate) => (
    candidate.id !== nodeId && currentDominators.has(candidate.id)
  ));
}
