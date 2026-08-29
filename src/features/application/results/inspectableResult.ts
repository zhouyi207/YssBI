import type { DeepReadonly } from '@/shared/types/deepReadonly';
import {
  outputPinRef,
  resultRef,
  type InspectableResultRef,
} from '@/features/domain/result/inspectableResultRef';
export { outputPinRef, resultRef };
export type { InspectableResultRef };
import type { PinResultEntry, ResultDescriptor } from './types';
import type { PinHistoryProjection } from '@/shared/types/ui/execution';
import type {
  ResultQueryCoordinator,
  ResultQueryReadCapability,
  ResultQueryOutcome,
} from './resultQueryCoordinator';

export interface InspectableResultQueryDependencies {
  readonly coordinator: ResultQueryCoordinator;
  readonly read: ResultQueryReadCapability;
}

export interface ResolvedInspectableResultRef {
  readonly ref: Extract<InspectableResultRef, { kind: 'result' }> | null;
  readonly history: DeepReadonly<PinHistoryProjection> | null;
  readonly status: ResultQueryOutcome['status'];
}

export async function resolveInspectableResultRef(
  ref: InspectableResultRef,
  dependencies: InspectableResultQueryDependencies,
  selectedResultId?: string | null,
): Promise<ResolvedInspectableResultRef> {
  if (ref.kind === 'result') {
    return { ref, history: null, status: 'published' };
  }

  const request = { graphPath: ref.graphPath, output: ref.output };
  const status = await dependencies.coordinator.loadPinHistory(request);
  if (status.status !== 'published') {
    return { ref: null, history: null, status: status.status };
  }

  const entries = dependencies.read.getPinHistory(request);
  if (!entries) return { ref: null, history: null, status: 'notReady' };
  const selected = selectedResultId == null
    ? entries[entries.length - 1]
    : entries.find((entry) => entry.resultId === selectedResultId);
  return {
    ref: selected ? resultRef(selected.resultId) as Extract<InspectableResultRef, { kind: 'result' }> : null,
    history: {
      graphPath: ref.graphPath,
      output: ref.output,
      entries,
      selectedResultId: selected?.resultId ?? null,
    },
    status: 'published',
  };
}

export async function resolveInspectableResult(
  ref: InspectableResultRef,
  dependencies: InspectableResultQueryDependencies,
  selectedResultId?: string | null,
): Promise<DeepReadonly<ResultDescriptor> | null> {
  const resolved = await resolveInspectableResultRef(ref, dependencies, selectedResultId);
  if (!resolved.ref) return null;

  const status = await dependencies.coordinator.loadDescriptor({
    resultId: resolved.ref.resultId,
  });
  if (status.status !== 'published') return null;
  return dependencies.read.getDescriptor(resolved.ref.resultId);
}

export type InspectableResultHistory = DeepReadonly<readonly PinResultEntry[]>;
