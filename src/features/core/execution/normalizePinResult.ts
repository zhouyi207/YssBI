import type { PinResultState } from '@/shared/types/ui';
import type { SourceDescriptor } from '@/features/core/resultSource';

export type PinResultWirePayload = {
  graphPath?: string;
  nodeId: string;
  pinId: string;
  sourceId: string;
  descriptor: SourceDescriptor;
};

/** Normalize IPC pin-result payloads into store shape (graphPath is canonical). */
export function normalizePinResultState(
  graphPath: string,
  payload: PinResultWirePayload,
): PinResultState {
  return {
    graphPath: payload.graphPath ?? graphPath,
    nodeId: payload.nodeId,
    pinId: payload.pinId,
    sourceId: payload.sourceId,
    descriptor: payload.descriptor,
  };
}

/** Pin results are bucketed per graphPath in ExecutionStore; size is authoritative. */
export function graphBucketHasPinResults(
  pinResults: ReadonlyMap<string, PinResultState>,
): boolean {
  return pinResults.size > 0;
}
