import { useExecutionStore } from '@/features/core/execution';

const IDLE = {
  nodeState: null,
  isCompleted: false,
  hasError: false,
} as const;

/**
 * Committed execution state from Zustand (after run/replay commit).
 * During live run / replay, visuals are imperative via executionVisualSession + CSS.
 */
export function useNodeExecution(nodeId: string, graphId?: string, enabled = true) {
  const nodeState = useExecutionStore((state) => {
    if (!enabled || !graphId) return null;
    return state.graphs[graphId]?.nodeStates.get(nodeId) ?? null;
  });
  const isCompleted = useExecutionStore((state) => {
    if (!enabled || !graphId) return false;
    return state.graphs[graphId]?.executedNodes.has(nodeId) ?? false;
  });

  if (!enabled || !graphId) return IDLE;

  const hasError = nodeState?.status === 'error';

  return {
    nodeState,
    isCompleted,
    hasError,
  };
}
