import {
  pinPreviewCacheKey,
  useExecutionStore,
  type PinPreviewLease,
} from '@/features/core/execution';
import type { GraphOutputRefDto } from '@/shared/types/dto/executionDemand';
import type { RunEvent, RunOutputChannelEvent } from '@/shared/types/dto/runEvent';

export type GraphRunOutcomeState = {
  outcome: 'success' | 'cancelled' | 'error';
};

export type PinPreviewObservation = {
  projectSessionId: string | null;
  output: GraphOutputRefDto;
  generation: number;
  runId: string | null;
  terminal: 'pending' | 'completed' | 'error' | 'cancelled';
  stale: boolean;
  lease: PinPreviewLease;
};

function observePinPreviewEvent(
  graphPath: string,
  event: RunEvent,
  preview: PinPreviewObservation,
): void {
  if (!preview.lease.isCurrent()) return;
  if (event.run.graphPath !== graphPath) {
    preview.stale = true;
    return;
  }

  if (event.kind.type === 'runStarted') {
    if (preview.runId) {
      preview.stale = true;
      return;
    }
    preview.projectSessionId = event.run.projectSessionId;
    preview.runId = event.run.runId;
    return;
  }
  if (
    !preview.projectSessionId
    || !preview.runId
    || event.run.projectSessionId !== preview.projectSessionId
    || event.run.runId !== preview.runId
  ) {
    preview.stale = true;
    return;
  }
  if (event.kind.type === 'runCompleted') {
    preview.terminal = 'completed';
    return;
  }
  if (event.kind.type === 'runErrored') {
    preview.terminal = 'error';
    return;
  }
  if (event.kind.type === 'runCancelled') {
    preview.terminal = 'cancelled';
    return;
  }
  if (event.kind.type !== 'pinPreviewResultReady') return;
  if (
    event.kind.generation !== preview.generation
    || event.kind.output.graphPath !== preview.output.graphPath
    || pinPreviewCacheKey(graphPath, event.kind.output.port)
      !== pinPreviewCacheKey(graphPath, preview.output.port)
  ) {
    preview.stale = true;
    return;
  }

  preview.lease.complete(event.kind.resultId);
}

export function observeGraphRunOutput(
  graphPath: string,
  event: RunOutputChannelEvent,
): void {
  useExecutionStore.getState().recordRunOutput(graphPath, event);
}

export function observeGraphRunEvent(
  graphPath: string,
  event: RunEvent,
  state: GraphRunOutcomeState,
  preview?: PinPreviewObservation,
): void {
  if (preview) {
    observePinPreviewEvent(graphPath, event, preview);
    return;
  }
  if (event.kind.type === 'runStarted') {
    useExecutionStore.getState().setActiveRunId(graphPath, event.run.runId);
  }
  if (event.kind.type === 'runErrored') state.outcome = 'error';
  if (event.kind.type === 'runCancelled') state.outcome = 'cancelled';
}
