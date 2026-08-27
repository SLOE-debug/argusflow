import { listen } from '@tauri-apps/api/event';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { useStore } from 'zustand';

import { createFlowStore } from '../../flow/store';
import type { FlowAnchorSide, FlowPoint } from '../../flow/types';
import type {
  ExecutionEvent,
  JsonObject,
  ValidationReport,
  WorkflowInputDefinition,
  WorkflowPermissions,
} from './contracts';
import {
  DEFAULT_EDGES,
  DEFAULT_NODES,
  DEFAULT_SELECTED_NODE_ID,
  DEFAULT_WORKFLOW_INPUTS,
  DEFAULT_WORKFLOW_NAME,
  DEFAULT_WORKFLOW_PERMISSIONS,
  DEFAULT_WORKFLOW_VARIABLES,
} from './defaultWorkflowTemplate';
import {
  applyExecutionEventToNodes,
  canConnect,
  createEdge,
  createNodeFromCreationKey,
  toWorkflowDefinition,
  type EditableNodeKind,
  type WorkflowNodeCreationKey,
  type WorkflowEdgeData,
  type WorkflowNodeData,
  type WorkflowNodeUpdater,
} from './workflowModel';
import { useWorkflowInputs } from './useWorkflowInputs';
import { useWorkflowComponents } from './useWorkflowComponents';
import {
  isDesktopRuntime,
  normalizeCommandError,
  runWorkflow,
  validateWorkflow,
} from './workflowApi';
import {
  bindConnectedResourceScope,
  oppositeAnchorSide,
} from './workflowResourceBinding';

const WORKFLOW_EVENT_NAME = 'argusflow://workflow-event';

/** 编排自研 Flow store、工作流设置和后端运行事件。 */
export function useWorkflowStudio() {
  const workflowId = useMemo(() => crypto.randomUUID(), []);
  const flowStore = useMemo(() => {
    /** 带参考工作流的独立画布 Store。 */
    const store = createFlowStore<WorkflowNodeData, WorkflowEdgeData>({
      metadata: {
        workflowName: DEFAULT_WORKFLOW_NAME,
        inputs: DEFAULT_WORKFLOW_INPUTS,
        variables: DEFAULT_WORKFLOW_VARIABLES,
        permissions: DEFAULT_WORKFLOW_PERMISSIONS,
      },
      nodes: DEFAULT_NODES,
      edges: DEFAULT_EDGES,
    });
    store.getState().selectNodes([DEFAULT_SELECTED_NODE_ID]);
    return store;
  }, []);
  const workflowName = useStore(
    flowStore,
    (state) => state.metadata.workflowName as string,
  );
  const variables = useStore(
    flowStore,
    (state) => state.metadata.variables as JsonObject,
  );
  const permissions = useStore(
    flowStore,
    (state) => state.metadata.permissions as WorkflowPermissions,
  );
  const nodes = useStore(flowStore, (state) => state.nodes);
  const workflowInputs = useWorkflowInputs(flowStore);
  const [variablesDraft, setVariablesDraft] = useState(
    JSON.stringify(DEFAULT_WORKFLOW_VARIABLES, null, 2),
  );
  const [variablesError, setVariablesError] = useState<string | null>(null);
  const [report, setReport] = useState<ValidationReport | null>(null);
  const [events, setEvents] = useState<ExecutionEvent[]>([]);
  const [running, setRunning] = useState(false);
  const [runId, setRunId] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const { componentCatalog, createComponent } = useWorkflowComponents(
    flowStore,
    setErrorMessage,
    setReport,
  );

  const setWorkflowName = useCallback((name: string) => {
    flowStore.getState().setMetadata(
      { workflowName: name },
      true,
      'workflow-name',
    );
  }, [flowStore]);

  const updatePermissions = useCallback((permissions: WorkflowPermissions) => {
    flowStore.getState().setMetadata(
      { permissions },
      true,
      'workflow-permissions',
    );
  }, [flowStore]);

  useEffect(() => {
    try {
      if (JSON.stringify(JSON.parse(variablesDraft)) !== JSON.stringify(variables)) {
        setVariablesDraft(JSON.stringify(variables, null, 2));
      }
    } catch {
      // 非法草稿必须保留给用户修正，不能被历史状态覆盖。
    }
  }, [variables, variablesDraft]);

  useEffect(() => {
    // 普通浏览器开发预览没有 Tauri IPC；只在桌面 WebView 中注册事件桥接。
    if (!isDesktopRuntime()) {
      return;
    }
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<ExecutionEvent>(WORKFLOW_EVENT_NAME, ({ payload }) => {
      setEvents((current) => [...current, payload]);
      const state = flowStore.getState();
      state.setNodes(applyExecutionEventToNodes(state.nodes, payload), false);
      if (payload.kind === 'edge_traversed' && payload.edge_id) {
        state.activateEdge(payload.edge_id);
      }
      if (
        payload.kind === 'workflow_completed'
        || payload.kind === 'workflow_failed'
      ) {
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
  }, [flowStore]);

  /** 从 Store 当前快照构造后端工作流，避免动作闭包订阅整图。 */
  const currentWorkflow = useCallback(() => {
    const state = flowStore.getState();
    return toWorkflowDefinition(
      workflowId,
      state.metadata.workflowName as string,
      state.metadata.inputs as WorkflowInputDefinition[],
      state.metadata.variables as JsonObject,
      state.metadata.permissions as WorkflowPermissions,
      state.nodes,
      state.edges,
    );
  }, [flowStore, workflowId]);

  const validate = useCallback(async () => {
    setErrorMessage(null);
    const draftError = variablesError ?? workflowInputs.inputDefinitionsError;
    if (draftError) {
      setErrorMessage(draftError);
      return null;
    }
    try {
      const nextReport = await validateWorkflow(
        currentWorkflow(),
        componentCatalog.map((item) => item.definition),
      );
      setReport(nextReport);
      /** 校验问题关联的节点 ID，用于一次性更新卡片错误状态。 */
      const invalidIds = new Set(nextReport.issues.flatMap((issue) => (
        issue.node_id ? [issue.node_id] : []
      )));
      const state = flowStore.getState();
      state.setNodes(state.nodes.map((node) => ({
        ...node,
        data: { ...node.data, invalid: invalidIds.has(node.id) },
      })), false);
      return nextReport;
    } catch (error) {
      setErrorMessage(normalizeCommandError(error).message);
      return null;
    }
  }, [componentCatalog, currentWorkflow, flowStore, variablesError, workflowInputs.inputDefinitionsError]);

  const run = useCallback(async () => {
    setEvents([]);
    setRunId(null);
    setErrorMessage(null);
    const state = flowStore.getState();
    state.setNodes(state.nodes.map((node) => ({
      ...node,
      data: { ...node.data, runState: 'idle', invalid: false },
    })), false);
    const nextReport = await validate();
    if (!nextReport?.valid) return;
    if (workflowInputs.runInputValuesError) {
      setErrorMessage(workflowInputs.runInputValuesError);
      return;
    }

    const validatedState = flowStore.getState();
    validatedState.setNodes(validatedState.nodes.map((node) => ({
      ...node,
      data: { ...node.data, runState: 'pending', invalid: false },
    })), false);
    setRunning(true);
    try {
      const started = await runWorkflow(
        currentWorkflow(),
        componentCatalog.map((item) => item.definition),
        { values: workflowInputs.runInputValues },
      );
      setRunId(started.run_id);
    } catch (error) {
      const commandError = normalizeCommandError(error);
      setErrorMessage(commandError.message);
      setRunning(false);
      const failedState = flowStore.getState();
      failedState.setNodes(failedState.nodes.map((node) => ({
        ...node,
        data: {
          ...node.data,
          runState: node.data.runState === 'pending'
            ? 'skipped'
            : node.data.runState,
        },
      })), false);
    }
  }, [componentCatalog, currentWorkflow, flowStore, validate, workflowInputs]);

  const addNode = useCallback((creationKey: WorkflowNodeCreationKey, position: FlowPoint) => {
    const state = flowStore.getState();
    const node = createNodeFromCreationKey(creationKey, position, componentCatalog);
    if (!node) return;
    const kind = node.kind as EditableNodeKind;
    if (
      (kind === 'start' || kind === 'end')
      && state.nodes.some((node) => node.kind === kind)
    ) {
      return;
    }
    state.transact((document) => ({
      ...document,
      nodes: [...document.nodes, node],
    }));
    flowStore.getState().selectNodes([node.id]);
    setReport(null);
  }, [componentCatalog, flowStore]);

  /** 在连线落点新建节点，并把节点与连线作为一次可撤销事务提交。 */
  const addConnectedNode = useCallback((
    creationKey: WorkflowNodeCreationKey,
    position: FlowPoint,
    sourceNodeId: string,
    sourceSide: FlowAnchorSide,
  ) => {
    const state = flowStore.getState();
    const createdNode = createNodeFromCreationKey(creationKey, position, componentCatalog);
    if (!createdNode) return false;
    const kind = createdNode.kind as EditableNodeKind;
    if (
      (kind === 'start' || kind === 'end')
      && state.nodes.some((node) => node.kind === kind)
    ) {
      return false;
    }

    const sourceNode = state.nodes.find((candidate) => candidate.id === sourceNodeId);
    /** 直接从 Application 拉出的 UI 节点默认绑定其 session 资源。 */
    const node = bindConnectedResourceScope(createdNode, sourceNode);
    const nodes = [...state.nodes, node];
    if (!canConnect(nodes, state.edges, sourceNodeId, node.id)) return false;

    /** 新节点默认从拖出方向的对侧接收连线。 */
    const targetSide = oppositeAnchorSide(sourceSide);
    const edge = createEdge(
      sourceNodeId,
      node.id,
      nodes,
      state.edges,
      sourceSide,
      targetSide,
    );
    state.transact((document) => ({
      ...document,
      nodes: [...document.nodes, node],
      edges: [...document.edges, edge],
    }));
    flowStore.getState().selectNodes([node.id]);
    setReport(null);
    return true;
  }, [componentCatalog, flowStore]);

  const connect = useCallback((
    source: string,
    target: string,
    sourceSide?: FlowAnchorSide,
    targetSide?: FlowAnchorSide,
  ) => {
    const state = flowStore.getState();
    if (!canConnect(state.nodes, state.edges, source, target)) return false;
    const edge = createEdge(
      source,
      target,
      state.nodes,
      state.edges,
      sourceSide,
      targetSide,
    );
    state.transact((document) => ({
      ...document,
      edges: [...document.edges, edge],
    }));
    setReport(null);
    return true;
  }, [flowStore]);

  const reconnect = useCallback((
    edgeId: string,
    endpoint: 'source' | 'target',
    nodeId: string,
    side?: FlowAnchorSide,
  ) => {
    const state = flowStore.getState();
    const edge = state.edges.find((candidate) => candidate.id === edgeId);
    if (!edge) return false;
    const source = endpoint === 'source' ? nodeId : edge.source.nodeId;
    const target = endpoint === 'target' ? nodeId : edge.target.nodeId;
    if (!canConnect(state.nodes, state.edges, source, target, edgeId)) {
      return false;
    }
    const sourceNode = state.nodes.find((node) => node.id === source);
    /** 当前源节点其他连线已经占用的条件分支。 */
    const usedBranches = new Set(state.edges
      .filter((candidate) => (
        candidate.id !== edgeId && candidate.source.nodeId === source
      ))
      .map((candidate) => candidate.data.branch));
    const branch = sourceNode?.kind === 'condition'
      ? edge.source.nodeId === source
        && edge.data.branch
        && !usedBranches.has(edge.data.branch)
        ? edge.data.branch
        : usedBranches.has('true') ? 'false' : 'true'
      : null;
    state.transact((document) => ({
      ...document,
      edges: document.edges.map((candidate) => candidate.id === edgeId
        ? {
            ...candidate,
            [endpoint]: { nodeId, side },
            data: { branch },
          }
        : candidate),
    }));
    return true;
  }, [flowStore]);

  /** 按稳定节点 ID 写回文档，使 Workspace 编辑不依赖当前 Inspector 选择。 */
  const updateNodeById = useCallback((
    nodeId: string,
    updater: WorkflowNodeUpdater,
  ) => {
    const state = flowStore.getState();
    if (!state.nodes.some((node) => node.id === nodeId)) return;
    state.transact((document) => ({
      ...document,
      nodes: document.nodes.map((node) => node.id === nodeId
        ? { ...node, data: updater(node.data) }
        : node),
    }), `node-fields:${nodeId}`);
    setReport(null);
  }, [flowStore]);

  /** Inspector 仍把字段修改应用到当前唯一选择。 */
  const updateNode = useCallback((updater: WorkflowNodeUpdater) => {
    const state = flowStore.getState();
    if (state.selectedNodeIds.size !== 1) return;
    const selectedNodeId = state.selectedNodeIds.values().next().value;
    if (selectedNodeId) {
      updateNodeById(selectedNodeId, updater);
    }
  }, [flowStore, updateNodeById]);

  const updateEdgeBranch = useCallback((branch: 'true' | 'false') => {
    const state = flowStore.getState();
    const selectedEdge = state.edges.find(
      (edge) => edge.id === state.selectedEdgeId,
    );
    if (!selectedEdge) return;
    state.transact((document) => {
      const conflict = document.edges.find((edge) => (
        edge.id !== selectedEdge.id
        && edge.source.nodeId === selectedEdge.source.nodeId
        && edge.data.branch === branch
      ));
      return {
        ...document,
        edges: document.edges.map((edge) => edge.id === selectedEdge.id
          ? { ...edge, data: { branch } }
          : edge.id === conflict?.id
            ? { ...edge, data: { branch: selectedEdge.data.branch } }
            : edge),
      };
    });
  }, [flowStore]);

  const updateVariables = useCallback((draft: string) => {
    setVariablesDraft(draft);
    try {
      const parsed: unknown = JSON.parse(draft);
      if (!parsed || Array.isArray(parsed) || typeof parsed !== 'object') {
        throw new Error('变量根值必须是 JSON 对象');
      }
      flowStore.getState().setMetadata(
        { variables: parsed as JsonObject },
        true,
        'workflow-variables',
      );
      setVariablesError(null);
    } catch (error) {
      setVariablesError(
        error instanceof Error ? error.message : 'JSON 格式无效',
      );
    }
  }, [flowStore]);

  const deleteSelection = useCallback(() => {
    flowStore.getState().deleteSelection();
  }, [flowStore]);

  return {
    flowStore,
    nodes,
    workflowName,
    setWorkflowName,
    permissions,
    updatePermissions,
    variablesDraft,
    variablesError,
    updateVariables,
    ...workflowInputs,
    report,
    events,
    running,
    runId,
    errorMessage,
    validate,
    run,
    addNode,
    addConnectedNode,
    connect,
    reconnect,
    updateNode,
    updateNodeById,
    updateEdgeBranch,
    deleteSelection,
    componentCatalog,
    createComponent,
  };
}
