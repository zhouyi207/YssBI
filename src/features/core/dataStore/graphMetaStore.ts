import { create } from 'zustand';
import { GraphId, NodeId } from '@/shared/types';
import { logger } from '@/utils/appLogger';

export interface GraphMeta {
  id: GraphId;
  name: string;
  type: 'event' | 'function' | 'macro';
  entryNodeId?: NodeId;
}

interface GraphMetaStore {
  // 图元信息表
  graphs: Record<GraphId, GraphMeta>;
  // 图顺序（用于 UI tab / 列表）
  graphOrder: GraphId[];

  // ==========================
  // CRUD
  // ==========================
  addGraph(meta: GraphMeta): void;
  updateGraph(id: GraphId, patch: Partial<GraphMeta>): void;
  deleteGraph(id: GraphId): void;

  // ==========================
  // Project / 全清
  // ==========================
  setGraphs(graphs: Record<GraphId, GraphMeta>, order?: GraphId[]): void;
  clear(): void;
}

export const useGraphMetaStore = create<GraphMetaStore>((set) => ({
  // ==========================
  // State
  // ==========================
  graphs: {},
  graphOrder: [],

  // ==========================
  // CRUD
  // ==========================
  addGraph: (meta) => set((state) => {
    if (state.graphs[meta.id]) {
      logger.data.warn(`addGraph: Graph "${meta.id}" already exists`, 'GraphMetaStore');
      return state;
    }

    return {
      graphs: { ...state.graphs, [meta.id]: meta },
      graphOrder: [...state.graphOrder, meta.id],
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

  // ==========================
  // Project / 全清
  // ==========================
  setGraphs: (graphs, order) => set({
    graphs: graphs ?? {},
    graphOrder: order ?? Object.keys(graphs ?? {}),
  }),

  clear: () => set({
    graphs: {},
    graphOrder: [],
  }),
}));
