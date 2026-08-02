import { useExecutionStore } from '@/features/core/execution';
import type { RunEvent } from '@/shared/types/dto/runEvent';

export type GraphRunOutcomeState = {
  outcome: 'success' | 'cancelled' | 'error';
};

export function observeGraphRunEvent(
  graphPath: string,
  event: RunEvent,
  state: GraphRunOutcomeState,
): void {
  if (event.kind.type === 'runStarted' && event.correlation.runId) {
    useExecutionStore.getState().setActiveRunId(graphPath, event.correlation.runId);
  }
  if (event.kind.type === 'runErrored') state.outcome = 'error';
  if (event.kind.type === 'runCancelled') state.outcome = 'cancelled';
}
