import type { GraphExecutionState } from "./executionTypes";

/** Clear frontend execution projections without touching backend result history. */
export function clearedRunProjectionsPatch(
  graphDirty = false,
): Pick<GraphExecutionState, "pinHistories" | "recording" | "graphDirty"> {
  return {
    pinHistories: new Map(),
    recording: [],
    graphDirty,
  };
}

/** Whether the graph still shows artifacts from a previous run (Clear button). */
export function graphHasClearableArtifacts(
  graph:
    | {
        status: GraphExecutionState["status"];
        pinHistories: { readonly size: number };
        recording: { readonly length: number };
        runOutput: {
          readonly entries: { readonly length: number };
          readonly projectionDropped: boolean;
        };
        nodeStates: { readonly size: number };
        completedConnections: { readonly size: number };
        flowingConnections: { readonly size: number };
      }
    | undefined,
): boolean {
  if (!graph) return false;
  if (graph.status === "running") return false;
  return (
    graph.pinHistories.size > 0 ||
    graph.recording.length > 0 ||
    graph.runOutput.entries.length > 0 ||
    graph.runOutput.projectionDropped ||
    graph.status === "completed" ||
    graph.status === "error" ||
    graph.nodeStates.size > 0 ||
    graph.completedConnections.size > 0 ||
    graph.flowingConnections.size > 0
  );
}
