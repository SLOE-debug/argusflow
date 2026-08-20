import type { Edge, Node } from '@xyflow/react';

import type { WorkflowDefinition, WorkflowNodeKind } from './contracts';
import type { ExecutionEvent } from './contracts';

/** 画布当前允许用户新增的节点类型；action 暂由后端契约保留。 */
export type EditableNodeKind = 'start' | 'log' | 'delay' | 'end';
/** 节点在一次运行中的可视状态。 */
export type NodeRunState = 'idle' | 'running' | 'success' | 'error';

/** React Flow 节点数据与工作流编辑字段的组合。 */
export type WorkflowNodeData = Record<string, unknown> & {
  /** 序列化时决定后端节点行为的类型。 */
  kind: EditableNodeKind;
  /** 画布卡片上展示的用户可读名称。 */
  label: string;
  /** Log 节点输出的文本内容。 */
  message?: string;
  /** Delay 节点等待时长，单位为毫秒。 */
  milliseconds?: number;
  /** 运行期间用于渲染状态指示灯的状态。 */
  runState?: NodeRunState;
  /** 校验失败时标记节点并显示错误样式。 */
  invalid?: boolean;
};

/** 带有 workflow 自定义数据的 React Flow 节点。 */
export type WorkflowCanvasNode = Node<WorkflowNodeData, 'workflow'>;
/** 工作流画布使用的 React Flow 边类型。 */
export type WorkflowCanvasEdge = Edge;

/** 首次打开编辑器时展示的线性示例节点。 */
export const DEFAULT_NODES: WorkflowCanvasNode[] = [
  canvasNode('start', 80, 'start', 'Start'),
  canvasNode('log', 320, 'log', 'Log', { message: 'ArgusFlow 已启动' }),
  canvasNode('delay', 560, 'delay', 'Delay', { milliseconds: 600 }),
  canvasNode('end', 800, 'end', 'End'),
];

/** 首次打开编辑器时连接示例节点的默认边。 */
export const DEFAULT_EDGES: WorkflowCanvasEdge[] = [
  workflowEdge('start', 'log'),
  workflowEdge('log', 'delay'),
  workflowEdge('delay', 'end'),
];

/**
 * 将画布状态转换为后端 Rust 命令所需的工作流契约。
 * @param workflowId 当前工作流 ID。
 * @param name 工作流名称。
 * @param nodes 画布节点集合。
 * @param edges 画布边集合。
 * @returns 不含 React Flow 展示字段的后端工作流定义。
 */
export function toWorkflowDefinition(
  workflowId: string,
  name: string,
  nodes: WorkflowCanvasNode[],
  edges: WorkflowCanvasEdge[],
): WorkflowDefinition {
  return {
    schema_version: 1,
    id: workflowId,
    name,
    nodes: nodes.map((node) => ({
      id: node.id,
      position: node.position,
      ...toNodeKind(node.data),
    })),
    edges: edges.map((edge) => ({
      id: edge.id,
      source: edge.source,
      target: edge.target,
    })),
  };
}

/**
 * 创建可编辑的 Log 或 Delay 节点，并为其生成全局唯一 ID。
 * @param kind 要创建的节点类型。
 * @param index 用于计算初始位置的当前节点数量或序号。
 * @returns 带默认参数和初始运行状态的画布节点。
 */
export function createNode(kind: 'log' | 'delay', index: number): WorkflowCanvasNode {
  const id = `${kind}-${crypto.randomUUID()}`;
  const extras =
    kind === 'log'
      ? { message: '记录一条运行信息' }
      : { milliseconds: 500 };

  return canvasNode(id, 260 + index * 36, kind, kind === 'log' ? 'Log' : 'Delay', extras, 120 + index * 28);
}

/**
 * 创建使用平滑折线渲染的有向画布边。
 * @param source 起始节点 ID。
 * @param target 目标节点 ID。
 * @returns 可直接交给 React Flow 的边对象。
 */
export function workflowEdge(source: string, target: string): WorkflowCanvasEdge {
  return {
    id: `${source}-${target}`,
    source,
    target,
    type: 'smoothstep',
  };
}

/**
 * 根据后端事件更新对应节点状态；工作流开始事件会重置所有节点标记。
 * @param nodes 当前画布节点。
 * @param event 后端推送的执行事件。
 * @returns 更新后的新节点数组，不修改输入节点。
 */
export function applyExecutionEventToNodes(
  nodes: WorkflowCanvasNode[],
  event: ExecutionEvent,
): WorkflowCanvasNode[] {
  if (event.kind === 'workflow_started') {
    return nodes.map((node) => ({
      ...node,
      data: { ...node.data, runState: 'idle', invalid: false },
    }));
  }

  // 只有节点级事件能改变单个节点状态，日志和工作流完成事件应保持节点原状态。
  const runState =
    event.kind === 'node_started'
      ? 'running'
      : event.kind === 'node_succeeded'
        ? 'success'
        : event.kind === 'node_failed'
          ? 'error'
          : null;
  if (!event.node_id || !runState) {
    return nodes;
  }

  return nodes.map((node) =>
    node.id === event.node_id
      ? { ...node, data: { ...node.data, runState } }
      : node,
  );
}

function canvasNode(
  id: string,
  x: number,
  kind: EditableNodeKind,
  label: string,
  extras: Partial<WorkflowNodeData> = {},
  y = 220,
): WorkflowCanvasNode {
  return {
    id,
    type: 'workflow',
    position: { x, y },
    data: {
      kind,
      label,
      runState: 'idle',
      ...extras,
    },
  };
}

function toNodeKind(data: WorkflowNodeData): WorkflowNodeKind {
  switch (data.kind) {
    case 'start':
      return { type: 'start' };
    case 'log':
      return { type: 'log', message: data.message ?? '' };
    case 'delay':
      return { type: 'delay', milliseconds: data.milliseconds ?? 0 };
    case 'end':
      return { type: 'end' };
  }
}
