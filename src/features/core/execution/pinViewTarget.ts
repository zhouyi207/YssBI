import { portAddressKey } from "@/features/domain/editorProjection";
import {
  outputPinRef,
  type InspectableResultRef,
} from "@/features/domain/result/inspectableResultRef";
import type {
  EditorConnectionProjectionDto,
  PortAddressDto,
} from "@/shared/types/domain/editorProjection";

export interface ResolvePinViewTargetParams {
  graphPath: string;
  address: PortAddressDto;
  direction: "input" | "output";
  connections?: readonly Pick<EditorConnectionProjectionDto, "input" | "output">[];
}

function sameAddress(left: PortAddressDto, right: PortAddressDto): boolean {
  return portAddressKey(left) === portAddressKey(right);
}

export function resolveUpstreamOutputs(
  input: PortAddressDto,
  connections: ResolvePinViewTargetParams["connections"],
): PortAddressDto[] {
  return (connections ?? [])
    .filter((connection) => sameAddress(connection.input, input))
    .map((connection) => connection.output);
}

export function inspectableRefsFromPinView(
  params: ResolvePinViewTargetParams,
): InspectableResultRef[] {
  const { graphPath, address, direction, connections } = params;
  const outputs = direction === "output" ? [address] : resolveUpstreamOutputs(address, connections);
  return outputs.map((output) => outputPinRef(graphPath, output));
}
