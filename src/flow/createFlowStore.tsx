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
  FlowDocumentSnapshot,
  FlowState,
} from './flowStoreTypes';
import { alignNodes, distributeNodes } from './selection';

const DEFAULT_EDGE_ACTIVATION_MS = 900;

/** 创建独立 Zustand Flow store，允许一个页面并列挂载多个编辑器。 */
export function createFlowStore<TData = unknown, TEdgeData = unknown>(
  initial?: Partial<FlowDocumentSnapshot<TData, TEdgeData>>,
): StoreApi<FlowState<TData, TEdgeData>> {
  return createStore<FlowState<TData, TEdgeData>>((set, get) => ({
    metadata: initial?.metadata ?? {},
    nodes: initial?.nodes ?? [],
    edges: initial?.edges ?? [],
    viewport: { x: 0, y: 42, zoom: 1 },
    selectedNodeIds: new Set(),
    selectedEdgeId: null,
    hoveredNodeId: null,
    hoveredEdgeId: null,
    selectionBox: null,
    connectionDraft: null,
    activeEdgeIds: {},
    past: [],
    future: [],
    clipboard: null,
    historyGroup: null,

    setViewport: (viewport) => set({ viewport }),

    setNodes: (nodes, record = true) => {
      if (record) {
        get().transact((document) => ({ ...document, nodes }));
      } else {
        set({ nodes });
      }
    },

    setEdges: (edges, record = true) => {
      if (record) {
        get().transact((document) => ({ ...document, edges }));
      } else {
        set({ edges });
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

    moveSelected: (delta, record = false) => {
      if (record) {
        get().transact((document) => ({
          ...document,
          nodes: moveSelectedNodes(
            document.nodes,
            get().selectedNodeIds,
            delta,
          ),
        }));
      } else {
        set((state) => ({
          nodes: moveSelectedNodes(
            state.nodes,
            state.selectedNodeIds,
            delta,
          ),
        }));
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
