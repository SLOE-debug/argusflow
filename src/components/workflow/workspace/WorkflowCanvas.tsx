import { useMemo } from 'react';
import type { StoreApi } from 'zustand';

import {
  FlowCanvas,
  FlowProvider,
  type FlowAnchorSide,
  type FlowPoint,
  type FlowState,
} from '../../../flow';
import type {
  WorkflowEdgeData,
  WorkflowNodeCreationKey,
  WorkflowNodeData,
} from '../../../features/workflow';
import { resolveCreationKind } from '../../../features/workflow';
import type { FlowComponentCatalogItem } from '../../../features/workflow';
import { FLOW_COMPONENT_CATALOG } from '../../../features/workflow';
import { NODE_PRESET_CATALOG } from '../../../features/workflow';
import { workflowNodeRegistry } from '../canvas/WorkflowNodeCard';
import { resolveWorkflowEdgeLabel } from '../presentation/workflowBranchPresentation';
import { WorkflowScopeBreadcrumbs } from './WorkflowScopeBreadcrumbs';

type WorkflowCanvasProps = {
  store: StoreApi<FlowState<WorkflowNodeData, WorkflowEdgeData>>;
  onAddNode: (kind: WorkflowNodeCreationKey, position: FlowPoint) => void;
  /** 新建节点并完成从现有节点开始的连线。 */
  onAddConnectedNode: (
    kind: WorkflowNodeCreationKey,
    position: FlowPoint,
    sourceNodeId: string,
    sourceSide: FlowAnchorSide,
  ) => boolean;
  onConnect: (
    source: string,
    target: string,
    sourceSide?: FlowAnchorSide,
    targetSide?: FlowAnchorSide,
  ) => boolean;
  onReconnect: (
    edgeId: string,
    endpoint: 'source' | 'target',
    nodeId: string,
    side?: FlowAnchorSide,
  ) => boolean;
  /** 双击流程组件时进入内部版本视图。 */
  onNodeDoubleClick?: (nodeId: string) => void;
  /** 放大到 While 内部时切换当前作用域。 */
  onEnterLoop?: (nodeId: string) => boolean;
  /** 面包屑切换到指定祖先作用域。 */
  onOpenScope?: (scopeId: string) => boolean;
  /** 删除结构容器时同步删除它拥有的子作用域树。 */
  onDeleteSelection?: () => void;
  /** 为工作区组件创建键补齐拖放尺寸定义。 */
  componentCatalog?: ReadonlyArray<FlowComponentCatalogItem>;
};

/** 将 ArgusFlow 节点注册表和业务约束接入自研 Flow 画布。 */
export function WorkflowCanvas({
  store,
  onAddNode,
  onAddConnectedNode,
  onConnect,
  onReconnect,
  onNodeDoubleClick,
  onEnterLoop,
  onOpenScope,
  onDeleteSelection,
  componentCatalog = FLOW_COMPONENT_CATALOG,
}: WorkflowCanvasProps) {
  /** 创建目录不变时复用注册表，保持画布键盘监听和节点定义引用稳定。 */
  const workflowCreationRegistry = useMemo(
    () => createWorkflowCreationRegistry(componentCatalog),
    [componentCatalog],
  );
  const addWorkflowNode = (kind: string, position: FlowPoint) => {
    if (isWorkflowCreationKey(kind)) onAddNode(kind, position);
  };

  const addConnectedWorkflowNode = (
    kind: string,
    position: FlowPoint,
    sourceNodeId: string,
    sourceSide: FlowAnchorSide,
  ) => isWorkflowCreationKey(kind) && onAddConnectedNode(
    kind,
    position,
    sourceNodeId,
    sourceSide,
  );

  return (
    <FlowProvider store={store}>
      <FlowCanvas
        edgeLabelResolver={resolveWorkflowEdgeLabel}
        registry={workflowCreationRegistry}
        onAddNode={addWorkflowNode}
        onAddConnectedNode={addConnectedWorkflowNode}
        onConnect={onConnect}
        onReconnect={onReconnect}
        onNodeDoubleClick={onNodeDoubleClick}
        onDeleteSelection={onDeleteSelection}
        onSemanticZoomIn={(worldPoint, nextZoom) => {
          if (nextZoom < 2.25 || !onEnterLoop) return false;
          const loop = store.getState().nodes.find((node) => (
            node.data.kind === 'loop'
            && worldPoint.x >= node.position.x
            && worldPoint.x <= node.position.x + node.size.width
            && worldPoint.y >= node.position.y
            && worldPoint.y <= node.position.y + node.size.height
          ));
          return loop ? onEnterLoop(loop.id) : false;
        }}
        onSemanticZoomOut={(nextZoom) => {
          if (nextZoom >= 0.1 || !onOpenScope) return false;
          const state = store.getState();
          const metadata = state.metadata.scopeMetadata as import('../../../features/workflow').WorkflowScopeMetadataMap;
          const parentScopeId = metadata[state.activeDocumentId]?.parent?.scope_id;
          return parentScopeId ? onOpenScope(parentScopeId) : false;
        }}
      />
      {onOpenScope ? (
        <WorkflowScopeBreadcrumbs
          store={store}
          onOpenScope={onOpenScope}
        />
      ) : null}
    </FlowProvider>
  );
}

/** 检查通用画布传入的注册键是否属于工作流领域节点。 */
function isWorkflowCreationKey(kind: string): kind is WorkflowNodeCreationKey {
  return resolveCreationKind(kind) !== null;
}

/** 通用 Flow 拖放需要按创建键查尺寸；Preset/Component 复用最终节点定义。 */
function createWorkflowCreationRegistry(
  componentCatalog: ReadonlyArray<FlowComponentCatalogItem>,
) {
  return {
    ...workflowNodeRegistry,
    ...Object.fromEntries(NODE_PRESET_CATALOG.map((preset) => [
      `preset:${preset.id}`,
      {
        ...workflowNodeRegistry.ui,
        kind: `preset:${preset.id}`,
        title: preset.title,
      },
    ])),
    ...Object.fromEntries(componentCatalog.map((item) => [
      `component:${item.definition.id}@${item.definition.version}`,
      {
        ...workflowNodeRegistry.component,
        kind: `component:${item.definition.id}@${item.definition.version}`,
        title: item.title,
      },
    ])),
  };
}
