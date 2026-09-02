import { create } from "zustand";
import { GraphPath, NodeId, type FunctionSignaturePin } from "@/shared/types";
import type { FunctionSignatureDto } from "@/shared/types/domain/editorMutation";
import { logger } from "@/features/core/observability/logger";

/** 函数签名投影（名称见 ResourceStore，图体见 GraphProjectionStore）。 */
export interface GraphMeta {
  path: GraphPath;
  name: string;
  type: "event" | "function";
  entryNodeId?: NodeId;
  functionRevision?: number;
  functionSignature?: FunctionSignatureDto;
  functionInputs?: FunctionSignaturePin[];
  functionOutputs?: FunctionSignaturePin[];
}

interface GraphMetaStore {
  graphs: Record<GraphPath, GraphMeta>;

  addGraph(meta: GraphMeta): void;
  updateGraph(id: GraphPath, patch: Partial<GraphMeta>): void;
  deleteGraph(id: GraphPath): void;

  setGraphs(graphs: Record<GraphPath, GraphMeta>): void;
  clear(): void;
}

export const useGraphMetaStore = create<GraphMetaStore>((set) => ({
  graphs: {},

  addGraph: (meta) =>
    set((state) => {
      if (state.graphs[meta.path]) {
        logger.data.warn(`addGraph: Graph "${meta.path}" already exists`, "GraphMetaStore");
        return state;
      }

      return {
        graphs: { ...state.graphs, [meta.path]: meta },
      };
    }),

  updateGraph: (id, patch) =>
    set((state) => {
      const prev = state.graphs[id];
      if (!prev) {
        logger.data.warn(`updateGraph: Graph "${id}" not found`, "GraphMetaStore");
        return state;
      }

      return {
        graphs: { ...state.graphs, [id]: { ...prev, ...patch } },
      };
    }),

  deleteGraph: (id) =>
    set((state) => {
      if (!state.graphs[id]) {
        logger.data.warn(`deleteGraph: Graph "${id}" not found`, "GraphMetaStore");
        return state;
      }

      const nextGraphs = { ...state.graphs };
      delete nextGraphs[id];

      return {
        graphs: nextGraphs,
      };
    }),

  setGraphs: (graphs) =>
    set({
      graphs: graphs ?? {},
    }),

  clear: () =>
    set({
      graphs: {},
    }),
}));
