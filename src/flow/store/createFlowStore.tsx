import { createStore, type StoreApi } from 'zustand';

import {
  copySelectedSubgraph,
  createPastedSubgraph,
  moveSelectedNodes,
  removeSelection,
} from './flowStoreDocument';
import {
  applyDocumentTransaction,
  redoDocument,
  undoDocument,
} from './flowStoreHistory';
import { updateNodeSelection } from './flowStoreSelection';
import type {
  FlowDocument,
  FlowDocumentSnapshot,
  FlowState,
} from './flowStoreTypes';
import { alignNodes, distributeNodes } from '../selection/selection';

/** 默认画布倍率，保持工作流文档原有的视图比例。 */
const DEFAULT_VIEWPORT = {
  x: 0,
  y: 42,
  zoom: 1,
} as const;

const DEFAULT_EDGE_ACTIVATION_MS = 900;
/** 没有显式多作用域配置时使用的根文档 ID。 */
const DEFAULT_DOCUMENT_ID = 'root';

/** 创建独立 Zustand Flow store，允许一个页面并列挂载多个编辑器。 */
export function createFlowStore<TData = unknown, TEdgeData = unknown>(
  initial?: Partial<FlowDocumentSnapshot<TData, TEdgeData>>,
): StoreApi<FlowState<TData, TEdgeData>> {
  /** 初始化时确保 active nodes/edges 与多文档表共享同一份不可变引用。 */
  const activeDocumentId = initial?.activeDocumentId ?? DEFAULT_DOCUMENT_ID;
  const initialDocument: FlowDocument<TData, TEdgeData> = {
    nodes: initial?.nodes ?? [],
    edges: initial?.edges ?? [],
  };
  const documents = initial?.documents ?? { [activeDocumentId]: initialDocument };
  const activeDocument = documents[activeDocumentId] ?? initialDocument;
  return createStore<FlowState<TData, TEdgeData>>((set, get) => ({
    metadata: initial?.metadata ?? {},
    nodes: activeDocument.nodes,
    edges: activeDocument.edges,
    documents,
    activeDocumentId,
    documentViewports: { [activeDocumentId]: { ...DEFAULT_VIEWPORT } },
    viewport: { ...DEFAULT_VIEWPORT },
    selectedNodeIds: new Set(),
    selectedEdgeId: null,
    hoveredNodeId: null,
    hoveredEdgeId: null,
    selectionBox: null,
    connectionDraft: null,
    routingInteraction: { kind: 'idle' },
    activeEdgeIds: {},
    past: [],
    future: [],
    clipboard: null,
    historyGroup: null,

    setViewport: (viewport) => set((state) => (
      state.viewport.x === viewport.x
      && state.viewport.y === viewport.y
      && state.viewport.zoom === viewport.zoom
        ? state
        : {
            viewport,
            documentViewports: {
              ...state.documentViewports,
              [state.activeDocumentId]: viewport,
            },
          }
    )),

    switchDocument: (documentId) => {
      const state = get();
      const document = state.documents[documentId];
      if (!document || documentId === state.activeDocumentId) return Boolean(document);
      set({
        activeDocumentId: documentId,
        nodes: document.nodes,
        edges: document.edges,
        viewport: state.documentViewports[documentId] ?? { ...DEFAULT_VIEWPORT },
        selectedNodeIds: new Set(),
        selectedEdgeId: null,
        hoveredNodeId: null,
        hoveredEdgeId: null,
        selectionBox: null,
        connectionDraft: null,
      });
      return true;
    },

    addDocument: (documentId, document) => {
      const state = get();
      if (!documentId.trim() || state.documents[documentId]) return false;
      state.transact((snapshot) => ({
        ...snapshot,
        documents: { ...snapshot.documents, [documentId]: document },
      }));
      return true;
    },

    removeDocuments: (documentIds) => {
      const state = get();
      if (documentIds.has(state.activeDocumentId)) return;
      state.transact((snapshot) => ({
        ...snapshot,
        documents: Object.fromEntries(Object.entries(snapshot.documents)
          .filter(([documentId]) => !documentIds.has(documentId))),
      }));
      set((current) => ({
        documentViewports: Object.fromEntries(Object.entries(current.documentViewports)
          .filter(([documentId]) => !documentIds.has(documentId))),
      }));
    },

    setNodes: (nodes, record = true) => {
      if (record) {
        get().transact((document) => ({ ...document, nodes }));
      } else {
        set((state) => state.nodes === nodes ? state : {
          nodes,
          documents: {
            ...state.documents,
            [state.activeDocumentId]: { nodes, edges: state.edges },
          },
        });
      }
    },

    setEdges: (edges, record = true) => {
      if (record) {
        get().transact((document) => ({ ...document, edges }));
      } else {
        set((state) => state.edges === edges ? state : {
          edges,
          documents: {
            ...state.documents,
            [state.activeDocumentId]: { nodes: state.nodes, edges },
          },
        });
      }
    },

    setMetadata: (metadata, record = true, historyGroup) => {
      if (record) {
        get().transact((document) => ({
          ...document,
          metadata: { ...document.metadata, ...metadata },
        }), historyGroup);
      } else {
        set((state) => ({
          metadata: { ...state.metadata, ...metadata },
        }));
      }
    },

    transact: (mutate, historyGroup) => set((state) => (
      applyDocumentTransaction(state, mutate, historyGroup)
    )),

    selectNodes: (ids, mode = 'replace') => set((state) => ({
      selectedNodeIds: updateNodeSelection(
        state.selectedNodeIds,
        ids,
        mode,
      ),
      selectedEdgeId: null,
    })),

    selectEdge: (selectedEdgeId) => set({
      selectedEdgeId,
      selectedNodeIds: new Set(),
    }),

    clearSelection: () => set({
      selectedNodeIds: new Set(),
      selectedEdgeId: null,
    }),

    setHoveredNode: (hoveredNodeId) => set({ hoveredNodeId }),
    setHoveredEdge: (hoveredEdgeId) => set({ hoveredEdgeId }),
    setSelectionBox: (selectionBox) => set({ selectionBox }),
    setConnectionDraft: (connectionDraft) => set({ connectionDraft }),
    setRoutingInteraction: (routingInteraction) => set({ routingInteraction }),

    moveSelected: (delta, record = false, historyGroup) => {
      if (delta.x === 0 && delta.y === 0) return;
      if (record) {
        get().transact((document) => ({
          ...document,
          nodes: moveSelectedNodes(
            document.nodes,
            get().selectedNodeIds,
            delta,
          ),
        }), historyGroup);
      } else {
        set((state) => {
          const nodes = moveSelectedNodes(
            state.nodes,
            state.selectedNodeIds,
            delta,
          );
          return {
            nodes,
            documents: {
              ...state.documents,
              [state.activeDocumentId]: { nodes, edges: state.edges },
            },
          };
        });
      }
    },

    align: (mode) => get().transact((document) => ({
      ...document,
      nodes: alignNodes(
        document.nodes,
        get().selectedNodeIds,
        mode,
      ),
    })),

    distribute: (mode) => get().transact((document) => ({
      ...document,
      nodes: distributeNodes(
        document.nodes,
        get().selectedNodeIds,
        mode,
      ),
    })),

    deleteSelection: (protectedKinds = new Set()) => {
      get().transact((document) => {
        const currentState = get();
        return removeSelection(
          document,
          currentState.selectedNodeIds,
          currentState.selectedEdgeId,
          protectedKinds,
        );
      });
    },

    copy: () => set((state) => ({
      clipboard: copySelectedSubgraph(state),
    })),

    paste: (singletonKinds = new Set()) => {
      const currentState = get();
      if (!currentState.clipboard) return;

      const pastedSubgraph = createPastedSubgraph(
        currentState.clipboard,
        currentState.nodes,
        singletonKinds,
      );
      get().transact((document) => ({
        ...document,
        nodes: [...document.nodes, ...pastedSubgraph.nodes],
        edges: [...document.edges, ...pastedSubgraph.edges],
      }));
      set({
        selectedNodeIds: new Set(
          pastedSubgraph.nodes.map((node) => node.id),
        ),
        selectedEdgeId: null,
        clipboard: pastedSubgraph,
      });
    },

    duplicate: (singletonKinds) => {
      get().copy();
      get().paste(singletonKinds);
    },

    undo: () => set((state) => undoDocument(state) ?? state),
    redo: () => set((state) => redoDocument(state) ?? state),

    activateEdge: (edgeId, duration = DEFAULT_EDGE_ACTIVATION_MS) => {
      const expires = Date.now() + duration;
      set((state) => ({
        activeEdgeIds: { ...state.activeEdgeIds, [edgeId]: expires },
      }));

      window.setTimeout(() => set((state) => {
        if (state.activeEdgeIds[edgeId] !== expires) return state;

        return {
          activeEdgeIds: Object.fromEntries(
            Object.entries(state.activeEdgeIds)
              .filter(([id]) => id !== edgeId),
          ),
        };
      }), duration + 20);
    },
  }));
}
