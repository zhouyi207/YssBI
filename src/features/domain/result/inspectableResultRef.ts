import type { PortAddressDto } from '@/shared/types/domain/editorProjection';

export type InspectableResultRef =
  | { readonly kind: 'result'; readonly resultId: string }
  | { readonly kind: 'outputPin'; readonly graphPath: string; readonly output: PortAddressDto };

export function resultRef(resultId: string): InspectableResultRef {
  return { kind: 'result', resultId };
}

export function outputPinRef(
  graphPath: string,
  output: PortAddressDto,
): InspectableResultRef {
  return { kind: 'outputPin', graphPath, output };
}
