import type { GraphExecutionState } from '@/shared/types/ui';

/** Last-run inspectable artifacts: pin index + replay recording. */
export function clearedRunArtifactsPatch(
  graphDirty = false,
): Pick<GraphExecutionState, 'pinResults' | 'recording' | 'graphDirty'> {
  return {
    pinResults: new Map(),
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
    graph.pinResults.size > 0
    || graph.recording.length > 0
    || graph.status === 'completed'
    || graph.status === 'error'
    || graph.nodeStates.size > 0
    || graph.completedConnections.size > 0
    || graph.flowingConnections.size > 0
  );
}
