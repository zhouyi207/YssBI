import { ResultService } from '@/services/result/resultService';
import type { PortAddressDto } from '@/shared/types/dto/editorProjection';
import type { ResultDescriptor } from '@/shared/types/dto/result';
import type { PinHistoryProjection } from '@/shared/types/ui';

export type InspectableResultRef =
  | { kind: 'result'; resultId: string }
  | { kind: 'outputPin'; graphPath: string; output: PortAddressDto };

export interface ResolvedInspectableResultRef {
  ref: Extract<InspectableResultRef, { kind: 'result' }> | null;
  history: PinHistoryProjection | null;
}

export function resultRef(resultId: string): InspectableResultRef {
  return { kind: 'result', resultId };
}

export function outputPinRef(
  graphPath: string,
  output: PortAddressDto,
): InspectableResultRef {
  return { kind: 'outputPin', graphPath, output };
}

export async function resolveInspectableResultRef(
  ref: InspectableResultRef,
  selectedResultId?: string | null,
): Promise<ResolvedInspectableResultRef> {
  if (ref.kind === 'result') {
    return { ref, history: null };
  }

  const entries = await ResultService.getPinHistory(ref.graphPath, ref.output);
  const selected = selectedResultId == null
    ? entries[entries.length - 1]
    : entries.find((entry) => entry.resultId === selectedResultId);
  return {
    ref: selected ? { kind: 'result', resultId: selected.resultId } : null,
    history: {
      graphPath: ref.graphPath,
      output: ref.output,
      entries,
      selectedResultId: selected?.resultId ?? null,
    },
  };
}

export async function resolveInspectableResult(
  ref: InspectableResultRef,
  selectedResultId?: string | null,
): Promise<ResultDescriptor | null> {
  const resolved = await resolveInspectableResultRef(ref, selectedResultId);
  return resolved.ref ? ResultService.getDescriptor(resolved.ref.resultId) : null;
}
