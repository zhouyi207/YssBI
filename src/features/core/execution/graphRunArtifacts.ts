import type { GraphExecutionState } from '@/shared/types/ui';

/** Clear frontend execution projections without touching backend result history. */
export function clearedRunProjectionsPatch(
  graphDirty = false,
): Pick<GraphExecutionState, 'pinHistories' | 'recording' | 'graphDirty'> {
  return {
    pinHistories: new Map(),
    recording: [],
    graphDirty,
  };
}

/** Whether the graph still shows artifacts from a previous run (Clear button). */
export function graphHasClearableArtifacts(
  graph: GraphExecutionState | undefined,
): boolean {
  if (!graph) return false;
  if (graph.status === 'running') return false;
  return (
    graph.pinHistories.size > 0
    || graph.recording.length > 0
    || graph.status === 'completed'
    || graph.status === 'error'
    || graph.nodeStates.size > 0
    || graph.completedConnections.size > 0
    || graph.flowingConnections.size > 0
  );
}
