import type { PinData } from "./graphRuntimeTypes";
import type { DeepReadonly } from "@/shared/types/deepReadonly";

/**
 * Copies the exact serializable Pin projection accepted by Canvas interaction state.
 * Component props and UI capabilities are deliberately excluded at this boundary.
 */
export function toInteractionPinData(pin: DeepReadonly<PinData>): PinData {
  return structuredClone({
    id: pin.id,
    nodeId: pin.nodeId,
    name: pin.name,
    type: pin.type,
    direction: pin.direction,
    dataType: pin.dataType,
    address: pin.address,
    display: pin.display,
    kind: pin.kind,
    orphan: pin.orphan,
    canRemove: pin.canRemove,
    connections: pin.connections,
    input: pin.input,
    resolvedType: pin.resolvedType,
    resolvedSchema: pin.resolvedSchema,
    status: pin.status,
  }) as PinData;
}
