import { create } from "zustand";
import type { ExecutionState, GraphExecutionState, RunFailureProjection } from "./executionTypes";

const emptyGraphState = (): GraphExecutionState => ({
  status: "idle",
  runId: null,
  request: null,
  runFailure: null,
});

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
  releaseGraphExecutionState: (graphPath) => {
    set((state) => {
      if (!state.graphs[graphPath]) return state;
      const graphs = { ...state.graphs };
      delete graphs[graphPath];
      return { graphs };
    });
  },
}));
