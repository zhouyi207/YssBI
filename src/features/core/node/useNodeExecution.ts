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
export function useNodeExecution(nodeId: string, graphPath?: string, enabled = true) {
  const nodeState = useExecutionStore((state) => {
    if (!enabled || !graphPath) return null;
    return state.graphs[graphPath]?.nodeStates.get(nodeId) ?? null;
  });

  if (!enabled || !graphPath) return IDLE;

  const hasError = nodeState?.status === 'error';
  const isCompleted = nodeState?.status === 'completed';

  return {
    nodeState,
    isCompleted,
    hasError,
  };
}
