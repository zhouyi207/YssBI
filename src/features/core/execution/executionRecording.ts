import { useExecutionStore } from './useExecutionStore';
import type { RecordedEvent } from '@/shared/types/ui/execution';

/** Prefer `executionComplete.hasError`; fall back to `nodeError` if the run aborted early. */
export function recordingHadError(recording: RecordedEvent[]): boolean {
  for (let i = recording.length - 1; i >= 0; i -= 1) {
    const event = recording[i].event;
    if (event.event === 'executionComplete') {
      return event.data.hasError;
    }
  }
  return recording.some((entry) => entry.event.event === 'nodeError');
}

export function firstNodeErrorMessage(recording: RecordedEvent[]): string | undefined {
  for (const entry of recording) {
    if (entry.event.event === 'nodeError') {
      return entry.event.data.error;
    }
  }
  return undefined;
}

/** `commitExecutionVisual` usually sets terminal status; this covers snapshot mismatch edge cases. */
export function ensureGraphExecutionTerminal(
  graphPath: string,
  outcome: 'success' | 'error',
): void {
  const store = useExecutionStore.getState();
  if (store.graphs[graphPath]?.status !== 'running') return;
  if (outcome === 'error') {
    store.failExecution(graphPath);
  } else {
    store.completeExecution(graphPath);
  }
}
