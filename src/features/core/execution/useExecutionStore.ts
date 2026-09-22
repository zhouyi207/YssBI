import { create } from "zustand";
import type { ExecutionState, GraphExecutionState, RunFailureProjection } from "./executionTypes";
import type { PortAddressDto } from "@/shared/types/domain/editorProjection";
import { pinPreviewCacheKey } from "./pinResultIndex";

const emptyGraphState = (): GraphExecutionState => ({
  status: "idle",
  runId: null,
  request: null,
  runFailure: null,
  pinPreviews: new Map(),
});

export interface PinPreviewLease {
  readonly generation: number;
  isCurrent: () => boolean;
  complete: (resultId: string) => boolean;
  fail: (error: string) => boolean;
  revoke: () => void;
}

type LeaseRecord = {
  graphPath: string;
  port: PortAddressDto;
  revoked: boolean;
  lease: PinPreviewLease;
};

const activePreviewLeases = new Map<string, LeaseRecord>();

function revokeGraphPreviewLeases(graphPath: string): void {
  for (const [key, record] of activePreviewLeases) {
    if (record.graphPath !== graphPath) continue;
    record.revoked = true;
    activePreviewLeases.delete(key);
  }
}

export function revokeAllPinPreviewLeases(): void {
  for (const record of activePreviewLeases.values()) record.revoked = true;
  activePreviewLeases.clear();
}

interface ExecutionStore extends ExecutionState {
  getGraph: (graphPath: string) => GraphExecutionState;
  startExecution: (graphPath: string) => () => boolean;
  submitExecution: (graphPath: string) => () => boolean;
  setActiveRunId: (graphPath: string, runId: string) => void;
  completeExecution: (graphPath: string) => void;
  failExecution: (graphPath: string) => void;
  markExecutionUnknown: (graphPath: string) => void;
  interruptExecution: (graphPath: string) => void;
  clearGraphRunProjections: (graphPath: string) => void;
  recordRunFailure: (graphPath: string, failure: RunFailureProjection) => void;
  clearRunFailure: (graphPath: string) => void;
  beginPinPreview: (graphPath: string, port: PortAddressDto, generation: number) => PinPreviewLease;
  completePinPreview: (
    graphPath: string,
    port: PortAddressDto,
    generation: number,
    resultId: string,
  ) => boolean;
  failPinPreview: (
    graphPath: string,
    port: PortAddressDto,
    generation: number,
    error: string,
  ) => boolean;
  removePinPreview: (graphPath: string, port: PortAddressDto, generation: number) => boolean;
  releaseGraphExecutionState: (graphPath: string) => void;
}

function updateGraph(
  state: ExecutionState,
  graphPath: string,
  patch: Partial<GraphExecutionState>,
) {
  return {
    graphs: {
      ...state.graphs,
      [graphPath]: { ...(state.graphs[graphPath] ?? emptyGraphState()), ...patch },
    },
  };
}

export const useExecutionStore = create<ExecutionStore>((set, get) => ({
  graphs: {},
  getGraph: (graphPath) => get().graphs[graphPath] ?? emptyGraphState(),
  submitExecution: (graphPath) => {
    const request = {};
    set((state) => updateGraph(state, graphPath, { request, runId: null, status: "submitting" }));
    return () => get().graphs[graphPath]?.request === request;
  },
  startExecution: (graphPath) => {
    const request = {};
    set((state) =>
      updateGraph(state, graphPath, { request, runId: null, runFailure: null, status: "running" }),
    );
    return () => get().graphs[graphPath]?.request === request;
  },
  setActiveRunId: (graphPath, runId) =>
    set((state) =>
      ["running", "unknown", "submitting"].includes(state.graphs[graphPath]?.status)
        ? updateGraph(state, graphPath, { runId, status: "running", runFailure: null })
        : state,
    ),
  completeExecution: (graphPath) =>
    set((state) =>
      updateGraph(state, graphPath, { status: "completed", runId: null, request: null }),
    ),
  markExecutionUnknown: (graphPath) =>
    set((state) => updateGraph(state, graphPath, { status: "unknown" })),
  failExecution: (graphPath) =>
    set((state) => updateGraph(state, graphPath, { status: "error", runId: null, request: null })),
  interruptExecution: (graphPath) => get().clearGraphRunProjections(graphPath),
  clearGraphRunProjections: (graphPath) =>
    set((state) =>
      state.graphs[graphPath]
        ? updateGraph(state, graphPath, {
            status: "idle",
            runId: null,
            request: null,
            runFailure: null,
          })
        : state,
    ),
  recordRunFailure: (graphPath, failure) =>
    set((state) => {
      const graph = state.graphs[graphPath];
      return graph?.status === "running" && graph.runId === failure.runId
        ? updateGraph(state, graphPath, { runFailure: failure })
        : state;
    }),
  clearRunFailure: (graphPath) =>
    set((state) =>
      state.graphs[graphPath] ? updateGraph(state, graphPath, { runFailure: null }) : state,
    ),
  beginPinPreview: (graphPath, port, generation) => {
    const key = pinPreviewCacheKey(graphPath, port);
    const previous = activePreviewLeases.get(key);
    if (previous) previous.revoked = true;

    let record!: LeaseRecord;
    const lease: PinPreviewLease = {
      generation,
      isCurrent: () => !record.revoked && activePreviewLeases.get(key) === record,
      complete: (resultId) =>
        lease.isCurrent() &&
        useExecutionStore.getState().completePinPreview(graphPath, port, generation, resultId),
      fail: (error) =>
        lease.isCurrent() &&
        useExecutionStore.getState().failPinPreview(graphPath, port, generation, error),
      revoke: () => {
        record.revoked = true;
        if (activePreviewLeases.get(key) === record) activePreviewLeases.delete(key);
      },
    };
    record = { graphPath, port, revoked: false, lease };
    activePreviewLeases.set(key, record);

    set((state) => {
      const graph = state.graphs[graphPath] ?? emptyGraphState();
      const pinPreviews = new Map(graph.pinPreviews);
      pinPreviews.set(key, {
        graphPath,
        port,
        generation,
        status: "pending",
        resultId: null,
        error: null,
      });
      return updateGraph(state, graphPath, { pinPreviews });
    });
    return lease;
  },

  completePinPreview: (graphPath, port, generation, resultId) => {
    let accepted = false;
    set((state) => {
      const graph = state.graphs[graphPath];
      if (!graph) return state;
      const key = pinPreviewCacheKey(graphPath, port);
      const preview = graph.pinPreviews.get(key);
      if (!preview || preview.generation !== generation) return state;
      const pinPreviews = new Map(graph.pinPreviews);
      pinPreviews.set(key, {
        ...preview,
        status: "ready",
        resultId,
        error: null,
      });
      accepted = true;
      return updateGraph(state, graphPath, { pinPreviews });
    });
    return accepted;
  },

  failPinPreview: (graphPath, port, generation, error) => {
    let accepted = false;
    set((state) => {
      const graph = state.graphs[graphPath];
      if (!graph) return state;
      const key = pinPreviewCacheKey(graphPath, port);
      const preview = graph.pinPreviews.get(key);
      if (!preview || preview.generation !== generation) return state;
      const pinPreviews = new Map(graph.pinPreviews);
      pinPreviews.set(key, {
        ...preview,
        status: "error",
        resultId: null,
        error,
      });
      accepted = true;
      return updateGraph(state, graphPath, { pinPreviews });
    });
    return accepted;
  },

  removePinPreview: (graphPath, port, generation) => {
    let removed = false;
    set((state) => {
      const graph = state.graphs[graphPath];
      if (!graph) return state;
      const key = pinPreviewCacheKey(graphPath, port);
      const preview = graph.pinPreviews.get(key);
      if (!preview || preview.generation !== generation) return state;
      const pinPreviews = new Map(graph.pinPreviews);
      pinPreviews.delete(key);
      removed = true;
      return updateGraph(state, graphPath, { pinPreviews });
    });
    return removed;
  },

  releaseGraphExecutionState: (graphPath) => {
    revokeGraphPreviewLeases(graphPath);
    set((state) => {
      if (!state.graphs[graphPath]) return state;
      const graphs = { ...state.graphs };
      delete graphs[graphPath];
      return { graphs };
    });
  },
}));
