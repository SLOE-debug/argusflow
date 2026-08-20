import { listen } from '@tauri-apps/api/event';
import {
  addEdge,
  useEdgesState,
  useNodesState,
  type Connection,
} from '@xyflow/react';
import { useEffect, useMemo, useState, type Dispatch, type SetStateAction } from 'react';

import type { ExecutionEvent, ValidationReport } from './contracts';
import {
  DEFAULT_EDGES,
  DEFAULT_NODES,
  applyExecutionEventToNodes,
  createNode,
  toWorkflowDefinition,
  type WorkflowCanvasEdge,
  type WorkflowCanvasNode,
} from './workflowModel';
import { normalizeCommandError, runWorkflow, validateWorkflow } from './workflowApi';

const WORKFLOW_EVENT_NAME = 'argusflow://workflow-event';

/** 管理工作流画布、后端校验/运行请求及实时执行状态的编辑器 Hook。 */
export function useWorkflowStudio() {
  /** 编辑器本次挂载期间保持稳定的工作流 ID，重新打开编辑器时重新生成。 */
  const workflowId = useMemo(() => crypto.randomUUID(), []);
  const [workflowName, setWorkflowName] = useState('我的第一个 ArgusFlow');
  const [nodes, setNodes, onNodesChange] = useNodesState<WorkflowCanvasNode>(DEFAULT_NODES);
  const [edges, setEdges, onEdgesChange] = useEdgesState(DEFAULT_EDGES);
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [selectedEdgeId, setSelectedEdgeId] = useState<string | null>(null);
  const [report, setReport] = useState<ValidationReport | null>(null);
  const [events, setEvents] = useState<ExecutionEvent[]>([]);
  const [running, setRunning] = useState(false);
  const [runId, setRunId] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    // 监听器可能在异步注册完成前卸载，因此用 disposed 防止卸载后的回调泄漏。
    void listen<ExecutionEvent>(WORKFLOW_EVENT_NAME, ({ payload }) => {
      setEvents((current) => [...current, payload]);
      setNodes((current) => applyExecutionEventToNodes(current, payload));
      if (payload.kind === 'workflow_completed' || payload.kind === 'workflow_failed') {
        setRunning(false);
      }
    }).then((stopListening) => {
      if (disposed) stopListening();
      else unlisten = stopListening;
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [setNodes]);

  const selectedNode = nodes.find((node) => node.id === selectedNodeId) ?? null;
  const canDelete =
    Boolean(selectedEdgeId) ||
    selectedNode?.data.kind === 'log' ||
    selectedNode?.data.kind === 'delay';
  /** 始终从最新画布状态生成请求，避免校验或运行时使用过期快照。 */
  const currentWorkflow = () =>
    toWorkflowDefinition(workflowId, workflowName, nodes, edges);

  const validate = async () => {
    setErrorMessage(null);
    try {
      const nextReport = await validateWorkflow(currentWorkflow());
      setReport(nextReport);
      markInvalidNodes(nextReport, setNodes);
      return nextReport;
    } catch (error) {
      setErrorMessage(normalizeCommandError(error).message);
      return null;
    }
  };

  const run = async () => {
    setEvents([]);
    setRunId(null);
    setErrorMessage(null);
    resetNodeStates(setNodes);
    const nextReport = await validate();
    if (!nextReport?.valid) return;

    setRunning(true);
    try {
      const started = await runWorkflow(currentWorkflow());
      setRunId(started.run_id);
    } catch (error) {
      const commandError = normalizeCommandError(error);
      setErrorMessage(commandError.message);
      if (commandError.issues.length > 0) {
        const nextReport = { valid: false, issues: commandError.issues };
        setReport(nextReport);
        markInvalidNodes(nextReport, setNodes);
      }
      setRunning(false);
    }
  };

  const connect = (connection: Connection) => {
    setEdges((current) =>
      addEdge(
        { ...connection, id: `${connection.source}-${connection.target}`, type: 'smoothstep' },
        current,
      ),
    );
    setReport(null);
  };

  /** 限制自环、固定端点方向和重复入/出边，使前端连线先满足后端的线性链约束。 */
  const isValidConnection = (connection: Connection | WorkflowCanvasEdge) => {
    if (!connection.source || !connection.target || connection.source === connection.target) {
      return false;
    }
    const sourceNode = nodes.find((node) => node.id === connection.source);
    const targetNode = nodes.find((node) => node.id === connection.target);
    return (
      sourceNode?.data.kind !== 'end' &&
      targetNode?.data.kind !== 'start' &&
      !edges.some((edge) => edge.source === connection.source) &&
      !edges.some((edge) => edge.target === connection.target)
    );
  };

  const addNode = (kind: 'log' | 'delay') => {
    const node = createNode(kind, nodes.length);
    setNodes((current) => [...current, node]);
    setSelectedNodeId(node.id);
    setSelectedEdgeId(null);
    setReport(null);
  };

  const updateNode = (data: Partial<WorkflowCanvasNode['data']>) => {
    if (!selectedNodeId) return;
    setNodes((current) =>
      current.map((node) =>
        node.id === selectedNodeId ? { ...node, data: { ...node.data, ...data } } : node,
      ),
    );
    setReport(null);
  };

  const deleteSelection = () => {
    if (selectedEdgeId) {
      setEdges((current) => current.filter((edge) => edge.id !== selectedEdgeId));
      setSelectedEdgeId(null);
      setReport(null);
      return;
    }
    if (!selectedNode || !['log', 'delay'].includes(selectedNode.data.kind)) return;
    setNodes((current) => current.filter((node) => node.id !== selectedNode.id));
    setEdges((current) =>
      current.filter(
        (edge) => edge.source !== selectedNode.id && edge.target !== selectedNode.id,
      ),
    );
    setSelectedNodeId(null);
    setReport(null);
  };

  return {
    workflowName,
    setWorkflowName,
    nodes,
    edges,
    onNodesChange,
    onEdgesChange,
    selectedNode,
    selectedEdgeId,
    setSelectedNodeId,
    setSelectedEdgeId,
    report,
    events,
    running,
    runId,
    errorMessage,
    canDelete,
    validate,
    run,
    connect,
    isValidConnection,
    addNode,
    updateNode,
    deleteSelection,
  };
}

/** 开始新一轮运行前清除上一次运行和校验留下的节点标记。 */
function resetNodeStates(setNodes: Dispatch<SetStateAction<WorkflowCanvasNode[]>>) {
  setNodes((current) =>
    current.map((node) => ({
      ...node,
      data: { ...node.data, runState: 'idle', invalid: false },
    })),
  );
}

/** 将后端报告中的节点问题投影到画布，供卡片显示错误边框。 */
function markInvalidNodes(
  report: ValidationReport,
  setNodes: Dispatch<SetStateAction<WorkflowCanvasNode[]>>,
) {
  const invalidIds = new Set(
    report.issues.flatMap((issue) => (issue.node_id ? [issue.node_id] : [])),
  );
  setNodes((current) =>
    current.map((node) => ({
      ...node,
      data: { ...node.data, invalid: invalidIds.has(node.id) },
    })),
  );
}
