import PanelRightClose from 'lucide-react/dist/esm/icons/panel-right-close.mjs';
import { useStore, type StoreApi } from 'zustand';

import type { FlowState } from '../../../flow';
import type { FlowComponentCatalogItem } from '../../../features/workflow';
import type {
  ControlPortId,
  WorkflowCanvasEdge,
  WorkflowCanvasNode,
  WorkflowEdgeData,
  WorkflowNodeData,
  WorkflowNodeUpdater,
} from '../../../features/workflow';
import type { WorkflowPermissions } from '../../../features/workflow';
import type {
  JsonObject,
  WorkflowInputDefinition,
} from '../../../features/workflow';
import { buildWorkflowSymbolRegistry } from '../../../features/workflow';
import {
  EdgeInspectorFields,
  MultipleSelection,
  NodeInspectorFields,
} from './NodeInspectorFields';
import { WorkflowInspectorFields } from './WorkflowInspectorFields';
import type { StructuredEditorTarget } from '../workspace/dock/structuredEditorTarget';
import { ValueExprEditorProvider } from './node-fields/ValueExprFields';
import { IconButton } from '../../ui';

type NodeInspectorProps = Readonly<{
  /** 属性面板按选择状态订阅的工作流 Store。 */
  store: StoreApi<FlowState<WorkflowNodeData, WorkflowEdgeData>>;
  /** 当前工作流名称。 */
  workflowName: string;
  /** 工作流系统能力声明。 */
  permissions: WorkflowPermissions;
  /** 当前工作区可解析的全部精确组件版本。 */
  componentCatalog?: ReadonlyArray<FlowComponentCatalogItem>;
  /** 修改工作流名称。 */
  onNameChange: (name: string) => void;
  /** 请求收起右侧属性面板；宽度由外层工作区保留。 */
  onCollapse: () => void;
  /** 修改工作流系统能力声明。 */
  onPermissionsChange: (permissions: WorkflowPermissions) => void;
  /** 打开工作流数据 Dock。 */
  onOpenWorkflowData?: () => void;
  /** 修改节点字段。 */
  onUpdateNode: (updater: WorkflowNodeUpdater) => void;
  /** 修改分支节点的控制端口。 */
  onUpdateEdgeBranch: (branch: ControlPortId) => void;
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
        ? `${selectedCount} 个节点`
        : '工作流';

  return (
    <aside className="z-10 flex h-full min-h-0 min-w-0 flex-col overflow-hidden border-l border-slate-200 bg-white">
      <header className="flex h-[34px] shrink-0 items-center border-b border-slate-200 bg-slate-50 px-3">
        <h2 className="text-[12px] font-semibold text-slate-800">属性</h2>
        <span className="ml-auto rounded bg-slate-200/70 px-1.5 py-0.5 text-[10px] leading-none text-slate-500">
          {inspectorContext}
        </span>
        <IconButton
          label="收起右侧面板"
          icon={PanelRightClose}
          size="compact"
          className="ml-1 shrink-0 border-slate-200 text-slate-500 hover:bg-slate-100 hover:text-slate-900"
          onClick={props.onCollapse}
        />
      </header>
      <div className="min-h-0 flex-1 overflow-y-auto">
        {!node && !edge && selectedCount <= 1 ? (
          <WorkflowInspectorFields
            workflowName={props.workflowName}
            permissions={props.permissions}
            onNameChange={props.onNameChange}
            onPermissionsChange={props.onPermissionsChange}
            onOpenWorkflowData={props.onOpenWorkflowData}
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
              symbols: buildWorkflowSymbolRegistry({
                inputs: workflowInputs,
                variables,
                nodes,
                edges,
                consumerNodeId: node.id,
              }),
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
            sourceData={nodes.find((candidate) => candidate.id === edge.source.nodeId)?.data ?? null}
            onBranchChange={props.onUpdateEdgeBranch}
            onDelete={props.onDelete}
          />
        ) : null}
      </div>
    </aside>
  );
}
