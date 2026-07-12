import { SourceService } from '@/services/resultSource/resultSourceService';
import type { SourceDescriptor } from './types';

/** Aligns with backend `ResultSourceStore` owner kinds. */
export type InspectableSourceRef =
  | { kind: 'runtimePin'; graphPath: string; pinId: string }
  | { kind: 'window'; sourceId: string };

export function runtimePinRef(graphPath: string, pinId: string): InspectableSourceRef {
  return { kind: 'runtimePin', graphPath, pinId };
}

export function windowSourceRef(sourceId: string): InspectableSourceRef {
  return { kind: 'window', sourceId };
}

/** Single resolver: runtime pin index is authoritative for canvas results. */
export async function resolveInspectableSource(
  ref: InspectableSourceRef,
): Promise<SourceDescriptor | null> {
  if (ref.kind === 'runtimePin') {
    return SourceService.getPinDescriptor(ref.graphPath, ref.pinId);
  }
  return SourceService.getDescriptor(ref.sourceId);
}
