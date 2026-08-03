import { pinPreviewCacheKey, useExecutionStore } from '@/features/core/execution';
import type { GraphOutputRefDto } from '@/shared/types/dto/executionDemand';
import type { RunEvent } from '@/shared/types/dto/runEvent';

export type GraphRunOutcomeState = {
  outcome: 'success' | 'cancelled' | 'error';
};

export type PinPreviewObservation = {
  projectSessionId: string | null;
  output: GraphOutputRefDto;
  generation: number;
  runId: string | null;
  terminal: 'pending' | 'completed' | 'error' | 'cancelled';
};

function observePinPreviewEvent(
  graphPath: string,
  event: RunEvent,
  preview: PinPreviewObservation,
): void {
  if (event.correlation.graphPath !== graphPath) return;

  if (event.kind.type === 'runStarted') {
    if (preview.runId || !event.correlation.runId) return;
    preview.projectSessionId = event.correlation.projectSessionId;
    preview.runId = event.correlation.runId;
    return;
  }
  if (
    !preview.projectSessionId
    || !preview.runId
    || event.correlation.projectSessionId !== preview.projectSessionId
    || event.correlation.runId !== preview.runId
  ) return;
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
  if (
    event.kind.type !== 'outputReady'
    || event.kind.output.graphPath !== preview.output.graphPath
    || pinPreviewCacheKey(graphPath, event.kind.output.port)
      !== pinPreviewCacheKey(graphPath, preview.output.port)
  ) return;

  useExecutionStore.getState().completePinPreview(
    graphPath,
    preview.output.port,
    preview.generation,
    event.kind.sourceId,
  );
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
  if (event.kind.type === 'runStarted' && event.correlation.runId) {
    useExecutionStore.getState().setActiveRunId(graphPath, event.correlation.runId);
  }
  if (event.kind.type === 'runErrored') state.outcome = 'error';
  if (event.kind.type === 'runCancelled') state.outcome = 'cancelled';
}
