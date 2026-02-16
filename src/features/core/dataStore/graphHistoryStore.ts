/**
 * Graph 编辑历史 Store
 * 为每个 graph 维护 undo/redo 栈
 */

import { create } from 'zustand';
import type { GraphId } from '@/shared/types';
import { getGraphById } from './projectHelpers';
import { useGraphDataStore } from './graphDataStore';

/** 可恢复的图快照（与 addGraphFromData 入参格式一致） */
export interface GraphSnapshot {
  nodes: any[];
  pins: any[];
  connections: { connections: Array<{ from_pin: string; to_pin: string }> };
}

interface GraphHistory {
  past: GraphSnapshot[];
  future: GraphSnapshot[];
}

const MAX_HISTORY = 50;

function createEmptyHistory(): GraphHistory {
  return { past: [], future: [] };
}

interface GraphHistoryStore {
  /** graphId -> history */
  histories: Record<GraphId, GraphHistory>;

  /** 清空所有历史（项目加载时调用） */
  clearAll: () => void;

  /** 保存当前 graph 状态到 past，清空 future */
  saveSnapshot: (graphId: GraphId) => void;

  /** 撤销 */
  undo: (graphId: GraphId) => boolean;

  /** 重做 */
  redo: (graphId: GraphId) => boolean;

  canUndo: (graphId: GraphId) => boolean;
  canRedo: (graphId: GraphId) => boolean;
}

function graphToSnapshot(graph: any): GraphSnapshot {
  const connections = (graph.connections || []).map((c: any) => ({
    from_pin: c.from,
    to_pin: c.to,
  }));
  return {
    nodes: graph.nodes ? [...graph.nodes] : [],
    pins: graph.pins ? [...graph.pins] : [],
    connections: { connections },
  };
}

export const useGraphHistoryStore = create<GraphHistoryStore>((set, get) => ({
  histories: {},

  clearAll: () => set({ histories: {} }),

  saveSnapshot: (graphId) => {
    const graph = getGraphById(graphId);
    if (!graph) return;

    const snapshot = graphToSnapshot(graph);

    set((state) => {
      const hist = state.histories[graphId] ?? createEmptyHistory();
      const past = [...hist.past, snapshot].slice(-MAX_HISTORY);
      return {
        histories: {
          ...state.histories,
          [graphId]: { past, future: [] },
        },
      };
    });
  },

  undo: (graphId) => {
    const hist = get().histories[graphId];
    if (!hist || hist.past.length === 0) return false;

    const graph = getGraphById(graphId);
    if (!graph) return false;

    const currentSnapshot = graphToSnapshot(graph);
    const prevSnapshot = hist.past[hist.past.length - 1];

    set((state) => {
      const h = state.histories[graphId];
      const past = h.past.slice(0, -1);
      const future = [currentSnapshot, ...(h.future || [])].slice(0, MAX_HISTORY);
      return {
        histories: {
          ...state.histories,
          [graphId]: { past, future },
        },
      };
    });

    useGraphDataStore.getState().clearGraph(graphId);
    useGraphDataStore.getState().addGraphFromData(graphId, {
      id: graphId,
      name: graph.name,
      type: graph.type,
      ...prevSnapshot,
    } as any);

    return true;
  },

  redo: (graphId) => {
    const hist = get().histories[graphId];
    if (!hist || hist.future.length === 0) return false;

    const graph = getGraphById(graphId);
    if (!graph) return false;

    const currentSnapshot = graphToSnapshot(graph);
    const nextSnapshot = hist.future[0];

    set((state) => {
      const h = state.histories[graphId];
      const past = [...(h.past || []), currentSnapshot].slice(-MAX_HISTORY);
      const future = h.future.slice(1);
      return {
        histories: {
          ...state.histories,
          [graphId]: { past, future },
        },
      };
    });

    useGraphDataStore.getState().clearGraph(graphId);
    useGraphDataStore.getState().addGraphFromData(graphId, {
      id: graphId,
      name: graph.name,
      type: graph.type,
      ...nextSnapshot,
    } as any);

    return true;
  },

  canUndo: (graphId) => {
    const hist = get().histories[graphId];
    return !!(hist && hist.past.length > 0);
  },

  canRedo: (graphId) => {
    const hist = get().histories[graphId];
    return !!(hist && hist.future.length > 0);
  },
}));
