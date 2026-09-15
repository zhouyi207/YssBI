import type { GraphExecutionState } from "./executionTypes";

/** Clear acknowledges the run summary; cached results retain their own lifecycle. */
export function graphHasClearableArtifacts(
  graph: Pick<GraphExecutionState, "status" | "runFailure"> | undefined,
): boolean {
  return Boolean(
    graph &&
    graph.status !== "running" &&
    (graph.status === "completed" || graph.status === "error" || graph.runFailure),
  );
}
