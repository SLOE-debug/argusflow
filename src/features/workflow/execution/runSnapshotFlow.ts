import {
  getNodesBounds,
  type FlowDocument,
  type FlowEdge,
  type FlowNode,
  type FlowRect,
} from '../../../flow';
import {
  WORKFLOW_LOOP_BODY_PADDING,
  WORKFLOW_LOOP_BODY_TOP_INSET,
  WORKFLOW_LOOP_PREVIEW_SCALE,
} from '../model/workflowLoopLayout';
import {
  isJsonObject,
  type WorkflowNodeContract,
  type RunPresentationSnapshot,
  type WorkflowDefinition,
} from '../model/contracts';
import type { NodeRunState } from '../model/workflowModel';

/** 只读运行快照节点只包含执行台渲染所需的数据。 */
type RunSnapshotNodeBase = Readonly<{
  label: string;
  typeId: string;
  runState: NodeRunState;
  executionCount: number;
}>;

/** While 在运行快照中保留其真实子作用域引用，供父画布直接展开子图。 */
export type RunSnapshotNodeData = RunSnapshotNodeBase & Readonly<{
  structure:
    | Readonly<{ type: 'atomic' }>
    | Readonly<{ type: 'loop'; bodyScopeId: string; maxIterations: number }>;
}>;

/** 只读运行快照边保留分支标签。 */
export type RunSnapshotEdgeData = Readonly<{
  branch: string | null;
}>;

export type RunSnapshotDocuments = Readonly<
  Record<string, FlowDocument<RunSnapshotNodeData, RunSnapshotEdgeData>>
>;

/** 将运行时 v10 定义转换为通用 FlowCanvas 可读取的多文档快照。 */
export function createRunSnapshotDocuments(
  workflow: WorkflowDefinition,
  presentation: RunPresentationSnapshot,
): RunSnapshotDocuments {
  return Object.fromEntries(workflow.graph.scopes.map((scope) => {
    const nodes: ReadonlyArray<FlowNode<RunSnapshotNodeData>> = scope.nodes.map((node) => ({
      id: node.id,
      kind: 'run_snapshot',
      position: node.position,
      size: node.size,
      data: {
        label: presentation.node_labels[node.id] ?? node.type_id,
        typeId: node.type_id,
        runState: 'pending',
        executionCount: 0,
        structure: resolveRunNodeStructure(workflow, scope.id, node),
      },
    }));
    const edges: ReadonlyArray<FlowEdge<RunSnapshotEdgeData>> = scope.edges.map((edge) => ({
      id: edge.id,
      source: { nodeId: edge.source },
      target: { nodeId: edge.target },
      data: { branch: edge.branch },
    }));
    return [scope.id, { nodes, edges }];
  }));
}

/** 返回运行节点在根画布内展开后的世界坐标，用于跨全部方向跟随嵌套节点。 */
export function findRunNodeDisplayBounds(
  workflow: WorkflowDefinition,
  nodeId: string,
): FlowRect | null {
  const scope = workflow.graph.scopes.find((candidate) => (
    candidate.nodes.some((node) => node.id === nodeId)
  ));
  const node = scope?.nodes.find((candidate) => candidate.id === nodeId);
  if (!scope || !node) return null;

  const scopeById = new Map(workflow.graph.scopes.map((candidate) => [candidate.id, candidate]));
  /** 已解析作用域的仿射变换，避免嵌套 While 重复遍历祖先。 */
  const transforms = new Map<string, ScopeTransform>();
  const resolving = new Set<string>();
  const resolveTransform = (scopeId: string): ScopeTransform | null => {
    const cached = transforms.get(scopeId);
    if (cached) return cached;
    if (scopeId === workflow.graph.root_scope_id) {
      const root = { x: 0, y: 0, scale: 1 };
      transforms.set(scopeId, root);
      return root;
    }
    if (resolving.has(scopeId)) return null;
    resolving.add(scopeId);
    const childScope = scopeById.get(scopeId);
    const parent = childScope?.parent;
    const parentScope = parent ? scopeById.get(parent.scope_id) : null;
    const owner = parentScope?.nodes.find((candidate) => candidate.id === parent?.node_id);
    const parentTransform = parent ? resolveTransform(parent.scope_id) : null;
    const childBounds = childScope ? getNodesBounds(childScope.nodes) : null;
    resolving.delete(scopeId);
    if (!owner || !parentTransform || !childBounds) return null;

    const scale = parentTransform.scale * WORKFLOW_LOOP_PREVIEW_SCALE;
    const transform = {
      x: parentTransform.x + parentTransform.scale * (
        owner.position.x
        + WORKFLOW_LOOP_BODY_PADDING
        - childBounds.x * WORKFLOW_LOOP_PREVIEW_SCALE
      ),
      y: parentTransform.y + parentTransform.scale * (
        owner.position.y
        + WORKFLOW_LOOP_BODY_TOP_INSET
        - childBounds.y * WORKFLOW_LOOP_PREVIEW_SCALE
      ),
      scale,
    };
    transforms.set(scopeId, transform);
    return transform;
  };
  const transform = resolveTransform(scope.id);
  if (!transform) return null;
  return {
    x: transform.x + node.position.x * transform.scale,
    y: transform.y + node.position.y * transform.scale,
    width: node.size.width * transform.scale,
    height: node.size.height * transform.scale,
  };
}

type ScopeTransform = Readonly<{ x: number; y: number; scale: number }>;

/** 从 v10 的结构关系和当前节点 payload 生成只读 While 展示信息。 */
function resolveRunNodeStructure(
  workflow: WorkflowDefinition,
  scopeId: string,
  node: WorkflowNodeContract,
): RunSnapshotNodeData['structure'] {
  const bodyScope = workflow.graph.scopes.find((candidate) => (
    candidate.parent?.scope_id === scopeId && candidate.parent.node_id === node.id
  ));
  if (!bodyScope || node.type_id !== 'argus.loop') return { type: 'atomic' };
  const maxIterations = isJsonObject(node.payload)
    && typeof node.payload.max_iterations === 'number'
    ? node.payload.max_iterations
    : 0;
  return {
    type: 'loop',
    bodyScopeId: bodyScope.id,
    maxIterations,
  };
}
