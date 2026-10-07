import type { PortAddressDto } from "@/shared/types/domain/editorProjection";
import { resultReference, type ResultReference } from "@/shared/types/domain/result";

export type InspectableResultRef =
  | ({ readonly kind: "result" } & ResultReference)
  | { readonly kind: "outputPin"; readonly graphPath: string; readonly output: PortAddressDto };

export function resultRef(reference: ResultReference): InspectableResultRef {
  return { kind: "result", ...resultReference(reference) };
}

export function outputPinRef(graphPath: string, output: PortAddressDto): InspectableResultRef {
  return { kind: "outputPin", graphPath, output };
}
