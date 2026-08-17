import type { RunOutputChannelEvent } from '@/shared/types/dto/runEvent';
import type { RunOutputProjection } from '@/shared/types/ui';

/** Backend emits at most 256 text events plus one status for each limit. */
export const RUN_OUTPUT_PROJECTION_MAX_ENTRIES = 258;

export function emptyRunOutputProjection(): RunOutputProjection {
  return {
    runId: null,
    entries: [],
    projectionDropped: false,
  };
}

export function appendRunOutput(
  projection: RunOutputProjection,
  event: RunOutputChannelEvent,
): RunOutputProjection {
  if (projection.runId !== null && projection.runId !== event.runId) return projection;
  const lastSequence = projection.entries[projection.entries.length - 1]?.sequence ?? 0;
  if (event.sequence <= lastSequence) return projection;

  const hasSequenceGap = event.sequence !== lastSequence + 1;
  const atCapacity = projection.entries.length >= RUN_OUTPUT_PROJECTION_MAX_ENTRIES;
  const entries = atCapacity
    ? [...projection.entries.slice(1), event]
    : [...projection.entries, event];
  return {
    runId: event.runId,
    entries,
    projectionDropped: projection.projectionDropped || hasSequenceGap || atCapacity,
  };
}
