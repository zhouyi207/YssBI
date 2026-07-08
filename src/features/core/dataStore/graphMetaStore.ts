import { create } from 'zustand';
import { GraphPath, NodeId, type FunctionSignaturePin } from '@/shared/types';
import { logger } from '@/utils/appLogger';

export interface GraphMeta {
  path: GraphPath;
  name: string;
  type: 'event' | 'function';
  entryNodeId?: NodeId;
  functionInputs?: FunctionSignaturePin[];
  functionOutputs?: FunctionSignaturePin[];
}

interface GraphMetaStore {
  graphs: Record<GraphPath, GraphMeta>;
  graphOrder: GraphPath[];

  addGraph(meta: GraphMeta): void;
  updateGraph(id: GraphPath, patch: Partial<GraphMeta>): void;
  deleteGraph(id: GraphPath): void;

  setGraphs(graphs: Record<GraphPath, GraphMeta>, order?: GraphPath[]): void;
  clear(): void;
}

export const useGraphMetaStore = create<GraphMetaStore>((set) => ({
  graphs: {},
  graphOrder: [],

  addGraph: (meta) => set((state) => {
    if (state.graphs[meta.path]) {
      logger.data.warn(`addGraph: Graph "${meta.path}" already exists`, 'GraphMetaStore');
      return state;
    }

    return {
      graphs: { ...state.graphs, [meta.path]: meta },
      graphOrder: [...state.graphOrder, meta.path],
    };
  }),

  updateGraph: (id, patch) => set((state) => {
    const prev = state.graphs[id];
    if (!prev) {
      logger.data.warn(`updateGraph: Graph "${id}" not found`, 'GraphMetaStore');
      return state;
    }

    return {
      graphs: { ...state.graphs, [id]: { ...prev, ...patch } },
    };
  }),

  deleteGraph: (id) => set((state) => {
    if (!state.graphs[id]) {
      logger.data.warn(`deleteGraph: Graph "${id}" not found`, 'GraphMetaStore');
      return state;
    }

    const nextGraphs = { ...state.graphs };
    delete nextGraphs[id];

    return {
      graphs: nextGraphs,
      graphOrder: state.graphOrder.filter(gid => gid !== id),
    };
  }),

  setGraphs: (graphs, order) => set({
    graphs: graphs ?? {},
    graphOrder: order ?? Object.keys(graphs ?? {}),
  }),

  clear: () => set({
    graphs: {},
    graphOrder: [],
  }),
}));
