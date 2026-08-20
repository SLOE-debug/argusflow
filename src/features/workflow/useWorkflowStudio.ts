import { listen } from '@tauri-apps/api/event';
import { useEffect, useMemo, useState } from 'react';
import { useStore } from 'zustand';

import { createFlowStore } from '../../flow/store';
import type { FlowAnchorSide, FlowPoint } from '../../flow/types';
import type { ExecutionEvent, JsonObject, ValidationReport } from './contracts';
import {
  DEFAULT_EDGES, DEFAULT_NODES, applyExecutionEventToNodes, canConnect, createEdge,
  createNode, toWorkflowDefinition, type EditableNodeKind, type WorkflowCanvasEdge,
  type WorkflowCanvasNode, type WorkflowEdgeData, type WorkflowNodeData,
} from './workflowModel';
import { normalizeCommandError, runWorkflow, validateWorkflow } from './workflowApi';

const WORKFLOW_EVENT_NAME = 'argusflow://workflow-event';

/** 编排自研 Flow store、工作流设置和后端运行事件。 */
export function useWorkflowStudio() {
  const workflowId = useMemo(() => crypto.randomUUID(), []);
  const flowStore = useMemo(() => createFlowStore<WorkflowNodeData, WorkflowEdgeData>({ metadata: { workflowName: '未命名工作流', variables: {} }, nodes: DEFAULT_NODES, edges: DEFAULT_EDGES }), []);
  const nodes = useStore(flowStore, (state) => state.nodes) as WorkflowCanvasNode[];
  const edges = useStore(flowStore, (state) => state.edges) as WorkflowCanvasEdge[];
  const selectedNodeIds = useStore(flowStore, (state) => state.selectedNodeIds);
  const selectedEdgeId = useStore(flowStore, (state) => state.selectedEdgeId);
  const workflowName = useStore(flowStore, (state) => state.metadata.workflowName as string);
  const variables = useStore(flowStore, (state) => state.metadata.variables as JsonObject);
  const [variablesDraft, setVariablesDraft] = useState('{}');
  const [variablesError, setVariablesError] = useState<string | null>(null);
  const [report, setReport] = useState<ValidationReport | null>(null);
  const [events, setEvents] = useState<ExecutionEvent[]>([]);
  const [running, setRunning] = useState(false);
  const [runId, setRunId] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const setWorkflowName = (name: string) => flowStore.getState().setMetadata({ workflowName: name }, true, 'workflow-name');

  useEffect(() => {
    try {
      if (JSON.stringify(JSON.parse(variablesDraft)) !== JSON.stringify(variables)) setVariablesDraft(JSON.stringify(variables, null, 2));
    } catch {
      // 非法草稿必须保留给用户修正，不能被历史状态覆盖。
    }
  }, [variables, variablesDraft]);

  useEffect(() => {
    // 普通浏览器开发预览没有 Tauri IPC；只在桌面 WebView 中注册事件桥接。
    if (!(window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<ExecutionEvent>(WORKFLOW_EVENT_NAME, ({ payload }) => {
      setEvents((current) => [...current, payload]);
      flowStore.setState((state) => ({ nodes: applyExecutionEventToNodes(state.nodes as WorkflowCanvasNode[], payload) }));
      if (payload.kind === 'edge_traversed' && payload.edge_id) flowStore.getState().activateEdge(payload.edge_id);
      if (payload.kind === 'workflow_completed' || payload.kind === 'workflow_failed') setRunning(false);
    }).then((stopListening) => disposed ? stopListening() : unlisten = stopListening);
    return () => { disposed = true; unlisten?.(); };
  }, [flowStore]);

  const selectedNode = selectedNodeIds.size === 1 ? nodes.find((node) => selectedNodeIds.has(node.id)) ?? null : null;
  const selectedEdge = edges.find((edge) => edge.id === selectedEdgeId) ?? null;
  const currentWorkflow = () => toWorkflowDefinition(workflowId, workflowName, variables, flowStore.getState().nodes as WorkflowCanvasNode[], flowStore.getState().edges as WorkflowCanvasEdge[]);

  const validate = async () => {
    setErrorMessage(null);
    if (variablesError) { setErrorMessage(variablesError); return null; }
    try {
      const nextReport = await validateWorkflow(currentWorkflow());
      setReport(nextReport);
      const invalidIds = new Set(nextReport.issues.flatMap((issue) => issue.node_id ? [issue.node_id] : []));
      flowStore.setState((state) => ({ nodes: state.nodes.map((node) => ({ ...node, data: { ...(node.data as WorkflowNodeData), invalid: invalidIds.has(node.id) } })) }));
      return nextReport;
    } catch (error) { setErrorMessage(normalizeCommandError(error).message); return null; }
  };

  const run = async () => {
    setEvents([]); setRunId(null); setErrorMessage(null);
    flowStore.setState((state) => ({ nodes: state.nodes.map((node) => ({ ...node, data: { ...(node.data as WorkflowNodeData), runState: 'idle', invalid: false } })) }));
    const nextReport = await validate();
    if (!nextReport?.valid) return;
    setRunning(true);
    try { const started = await runWorkflow(currentWorkflow()); setRunId(started.run_id); }
    catch (error) { const commandError = normalizeCommandError(error); setErrorMessage(commandError.message); setRunning(false); }
  };

  const addNode = (kind: EditableNodeKind, position?: FlowPoint) => {
    if ((kind === 'start' || kind === 'end') && nodes.some((node) => node.kind === kind)) return;
    /** 面板点击没有鼠标世界坐标，因此以可见起始区为基准交错放置，避免节点完全重叠。 */
    const initialPosition = position ?? { x: 80 + nodes.length % 3 * 230, y: 110 + Math.floor(nodes.length / 3) * 180 };
    const node = createNode(kind, initialPosition);
    flowStore.getState().transact((state) => ({ ...state, nodes: [...state.nodes, node] }));
    flowStore.getState().selectNodes([node.id]);
    setReport(null);
  };

  const connect = (source: string, target: string, sourceSide?: FlowAnchorSide, targetSide?: FlowAnchorSide) => {
    const state = flowStore.getState();
    if (!canConnect(state.nodes as WorkflowCanvasNode[], state.edges as WorkflowCanvasEdge[], source, target)) return false;
    const edge = createEdge(source, target, state.nodes as WorkflowCanvasNode[], state.edges as WorkflowCanvasEdge[], sourceSide, targetSide);
    state.transact((snapshot) => ({ ...snapshot, edges: [...snapshot.edges, edge] }));
    setReport(null);
    return true;
  };

  const reconnect = (edgeId: string, endpoint: 'source' | 'target', nodeId: string, side?: FlowAnchorSide) => {
    const edge = edges.find((candidate) => candidate.id === edgeId);
    if (!edge) return false;
    const source = endpoint === 'source' ? nodeId : edge.source.nodeId;
    const target = endpoint === 'target' ? nodeId : edge.target.nodeId;
    if (!canConnect(nodes, edges, source, target, edgeId)) return false;
    const newSourceNode = nodes.find((node) => node.id === source);
    const usedBranches = new Set(edges.filter((candidate) => candidate.id !== edgeId && candidate.source.nodeId === source).map((candidate) => candidate.data.branch));
    const branch = newSourceNode?.kind === 'condition'
      ? edge.source.nodeId === source && edge.data.branch && !usedBranches.has(edge.data.branch)
        ? edge.data.branch
        : usedBranches.has('true') ? 'false' : 'true'
      : null;
    flowStore.getState().transact((state) => ({ ...state, edges: state.edges.map((candidate) => candidate.id === edgeId ? { ...candidate, [endpoint]: { nodeId, side }, data: { branch } } : candidate) }));
    return true;
  };

  const updateNode = (data: Partial<WorkflowNodeData>) => {
    if (!selectedNode) return;
    flowStore.getState().transact((state) => ({ ...state, nodes: state.nodes.map((node) => node.id === selectedNode.id ? { ...node, data: { ...(node.data as WorkflowNodeData), ...data } } : node) }), `node-fields:${selectedNode.id}`);
    setReport(null);
  };

  const updateEdgeBranch = (branch: 'true' | 'false') => {
    if (!selectedEdge) return;
    flowStore.getState().transact((state) => {
      const conflict = state.edges.find((edge) => edge.id !== selectedEdge.id && edge.source.nodeId === selectedEdge.source.nodeId && (edge.data as WorkflowEdgeData).branch === branch);
      return { ...state, edges: state.edges.map((edge) => edge.id === selectedEdge.id ? { ...edge, data: { branch } } : edge.id === conflict?.id ? { ...edge, data: { branch: selectedEdge.data.branch } } : edge) };
    });
  };

  const updateVariables = (draft: string) => {
    setVariablesDraft(draft);
    try {
      const parsed: unknown = JSON.parse(draft);
      if (!parsed || Array.isArray(parsed) || typeof parsed !== 'object') throw new Error('变量根值必须是 JSON 对象');
      flowStore.getState().setMetadata({ variables: parsed as JsonObject }, true, 'workflow-variables'); setVariablesError(null);
    } catch (error) { setVariablesError(error instanceof Error ? error.message : 'JSON 格式无效'); }
  };

  return {
    flowStore, workflowName, setWorkflowName, variablesDraft, variablesError, updateVariables,
    nodes, edges, selectedNode, selectedNodeIds, selectedEdge, selectedEdgeId, report, events,
    running, runId, errorMessage, validate, run, addNode, connect, reconnect, updateNode,
    updateEdgeBranch, deleteSelection: () => flowStore.getState().deleteSelection(),
  };
}
