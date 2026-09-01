import {
  getNodesBounds,
  type FlowDocument,
  type FlowNode,
  type FlowRect,
} from '../../../flow';

import type {
  WorkflowEdgeData,
  WorkflowNodeData,
} from './workflowModel';

/** 子图在父级 While 中按真实节点外观缩放后的展示倍率。 */
export const WORKFLOW_LOOP_PREVIEW_SCALE = 0.68;

/** 子图左右和底部与 While 外框之间保留的紧凑逻辑留白。 */
export const WORKFLOW_LOOP_BODY_PADDING = 6;

/** 内置标签下方为真实子图保留的最小顶部空间。 */
export const WORKFLOW_LOOP_BODY_TOP_INSET = 20;

/** 空子图和极小子图仍需保留可进入、可连接的最小命中区域。 */
export const WORKFLOW_LOOP_MIN_SIZE = {
  width: 220,
  height: 120,
} as const;

/** While 正文的原始边界和由它推导出的父级容器尺寸。 */
export type WorkflowLoopLayout = Readonly<{
  bounds: FlowRect | null;
  size: Readonly<{ width: number; height: number }>;
}>;

/** 多作用域工作流文档表。 */
export type WorkflowLoopDocuments = Readonly<Record<
  string,
  FlowDocument<WorkflowNodeData, WorkflowEdgeData>
>>;

/**
 * 根据直接子节点的真实位置和尺寸计算 While 外框。
 *
 * 位置原点会在预览中归一化，因此负坐标和远离原点的子图都只按实际跨度占用空间。
 */
export function resolveWorkflowLoopLayout(
  nodes: ReadonlyArray<FlowNode>,
): WorkflowLoopLayout {
  const bounds = getNodesBounds(nodes);
  if (!bounds) {
    return { bounds: null, size: { ...WORKFLOW_LOOP_MIN_SIZE } };
  }

  return {
    bounds,
    size: {
      width: Math.max(
        WORKFLOW_LOOP_MIN_SIZE.width,
        Math.ceil(
          bounds.width * WORKFLOW_LOOP_PREVIEW_SCALE
          + WORKFLOW_LOOP_BODY_PADDING * 2,
        ),
      ),
      height: Math.max(
        WORKFLOW_LOOP_MIN_SIZE.height,
        Math.ceil(
          bounds.height * WORKFLOW_LOOP_PREVIEW_SCALE
          + WORKFLOW_LOOP_BODY_TOP_INSET
          + WORKFLOW_LOOP_BODY_PADDING,
        ),
      ),
    },
  };
}

/**
 * 从最深层子作用域向外同步全部 While 尺寸。
 *
 * 尺寸是子图的派生数据；没有任何尺寸变化时保留原文档表引用，供 Store 避免空更新。
 */
export function synchronizeWorkflowLoopContainerSizes(
  documents: WorkflowLoopDocuments,
): WorkflowLoopDocuments {
  /** 已完成深度同步的作用域，避免嵌套 While 被重复计算。 */
  const resolvedDocuments = new Map<string, FlowDocument<WorkflowNodeData, WorkflowEdgeData>>();
  /** 防御非法循环作用域引用；合法文档是一棵有根作用域树。 */
  const resolvingScopeIds = new Set<string>();
  let documentsChanged = false;

  const resolveDocument = (
    scopeId: string,
  ): FlowDocument<WorkflowNodeData, WorkflowEdgeData> | undefined => {
    const resolved = resolvedDocuments.get(scopeId);
    if (resolved) return resolved;

    const document = documents[scopeId];
    if (!document || resolvingScopeIds.has(scopeId)) return document;
    resolvingScopeIds.add(scopeId);

    let nodesChanged = false;
    const nodes = document.nodes.map((node) => {
      if (node.data.kind !== 'loop') return node;

      const bodyDocument = resolveDocument(node.data.bodyScopeId);
      const size = resolveWorkflowLoopLayout(bodyDocument?.nodes ?? []).size;
      if (
        node.size.width === size.width
        && node.size.height === size.height
      ) return node;

      nodesChanged = true;
      return { ...node, size };
    });

    resolvingScopeIds.delete(scopeId);
    const resolvedDocument = nodesChanged ? { ...document, nodes } : document;
    if (resolvedDocument !== document) documentsChanged = true;
    resolvedDocuments.set(scopeId, resolvedDocument);
    return resolvedDocument;
  };

  for (const scopeId of Object.keys(documents)) resolveDocument(scopeId);
  if (!documentsChanged) return documents;

  return Object.fromEntries(Object.keys(documents).map((scopeId) => [
    scopeId,
    resolvedDocuments.get(scopeId) ?? documents[scopeId],
  ]));
}
