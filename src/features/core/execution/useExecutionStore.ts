import { create } from "zustand";
import { castDraft, produce } from "immer";
import { graphOutputKey } from "@/features/domain/editorProjection";
import type { RunEvent } from "@/shared/types/domain/runEvent";
import type { GraphOutputRefDto } from "@/shared/types/domain/executionDemand";
import type { ExecutionState, GraphExecutionState } from "./executionTypes";

const emptyGraphState = (): GraphExecutionState => ({
  status: "idle",
  run: null,
  request: null,
  runFailure: null,
  outputRuns: {},
});

interface ExecutionStore extends ExecutionState {
  getRunEventOutputs: (event: RunEvent) => GraphOutputRefDto[];
  applyRunEvent: (event: RunEvent, updateRunStatus?: boolean) => void;
  clearOutputRuns: (graphPath?: string) => void;
  getGraph: (graphPath: string) => GraphExecutionState;
  submitExecution: (graphPath: string) => () => boolean;
  markExecutionUnknown: (graphPath: string) => void;
  interruptExecution: (graphPath: string) => void;
  clearGraphRunProjections: (graphPath: string) => void;
  clearRunFailure: (graphPath: string) => void;
  releaseGraphExecutionState: (graphPath: string) => void;
}

function updateGraph(
  state: ExecutionState,
  graphPath: string,
  patch: Partial<GraphExecutionState>,
) {
  return produce(state, (draft) => {
    const graph = (draft.graphs[graphPath] ??= emptyGraphState());
    Object.assign(graph, patch);
  });
}

export const useExecutionStore = create<ExecutionStore>((set, get) => ({
  graphs: {},
  // Query invalidation and the final write share output ownership selection. The write
  // rechecks after synchronous query listeners have run.
  getRunEventOutputs: (event) => {
    const runs = get().graphs[event.run.graphPath]?.outputRuns ?? {};
    if (event.kind.type === "runStarted")
      return event.kind.outputs.filter((output) => {
        const previous = runs[graphOutputKey(output)];
        return (
          !previous ||
          previous.run.executionSessionId !== event.run.executionSessionId ||
          BigInt(previous.run.runId) < BigInt(event.run.runId)
        );
      });
    if (event.kind.type === "resultInspectionRequested") return [];
    return Object.values(runs)
      .filter(
        (run) =>
          run.active &&
          run.run.executionSessionId === event.run.executionSessionId &&
          run.run.runId === event.run.runId,
      )
      .map((run) => run.output);
  },
  applyRunEvent: (event, updateRunStatus = true) => {
    const outputs = get().getRunEventOutputs(event);
    set(
      produce((state: ExecutionState) => {
        const path = event.run.graphPath;
        if (event.kind.type === "resultInspectionRequested") return;
        if (!state.graphs[path] && event.kind.type !== "runStarted") return;
        const graph = (state.graphs[path] ??= emptyGraphState());
        if (event.kind.type === "runStarted") {
          for (const output of outputs) {
            const key = graphOutputKey(output);
            graph.outputRuns[key] = castDraft({
              run: event.run,
              output,
              active: true,
              resultRevision: event.resultRevision,
            });
          }
          if (updateRunStatus) {
            if (
              !["running", "submitting", "unknown"].includes(graph.status) ||
              (graph.run !== null &&
                (graph.run.executionSessionId !== event.run.executionSessionId ||
                  graph.run.runId !== event.run.runId))
            )
              graph.request = {};
            graph.status = "running";
            graph.run = castDraft(event.run);
            graph.runFailure = null;
          }
          return;
        }
        for (const output of outputs) {
          const run = graph.outputRuns[graphOutputKey(output)];
          run.active = false;
          run.resultRevision = event.resultRevision;
        }
        if (
          !updateRunStatus ||
          graph.run?.executionSessionId !== event.run.executionSessionId ||
          graph.run.runId !== event.run.runId
        )
          return;
        graph.status =
          event.kind.type === "runCompleted"
            ? "completed"
            : event.kind.type === "runErrored"
              ? "error"
              : "idle";
        graph.run = null;
        graph.request = null;
        graph.runFailure =
          event.kind.type === "runErrored"
            ? {
                run: castDraft(event.run),
                code: event.kind.code,
                phase: event.kind.phase,
                source: castDraft(event.kind.source),
                incidentId: null,
              }
            : null;
      }),
    );
  },
  clearOutputRuns: (graphPath) =>
    set(
      produce((state: ExecutionState) => {
        for (const [path, graph] of Object.entries(state.graphs))
          if (
            (graphPath === undefined || graphPath === path) &&
            Object.keys(graph.outputRuns).length
          )
            graph.outputRuns = {};
      }),
    ),
  getGraph: (graphPath) => get().graphs[graphPath] ?? emptyGraphState(),
  submitExecution: (graphPath) => {
    const request = {};
    set((state) => updateGraph(state, graphPath, { request, run: null, status: "submitting" }));
    return () => get().graphs[graphPath]?.request === request;
  },
  markExecutionUnknown: (graphPath) =>
    set((state) => updateGraph(state, graphPath, { status: "unknown" })),
  interruptExecution: (graphPath) => get().clearGraphRunProjections(graphPath),
  clearGraphRunProjections: (graphPath) =>
    set((state) =>
      state.graphs[graphPath]
        ? updateGraph(state, graphPath, {
            status: "idle",
            run: null,
            request: null,
            runFailure: null,
          })
        : state,
    ),
  clearRunFailure: (graphPath) =>
    set((state) =>
      state.graphs[graphPath]?.runFailure
        ? updateGraph(state, graphPath, { runFailure: null })
        : state,
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
