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
    direction: pin.direction,
    address: pin.address,
    display: pin.display,
    orphan: pin.orphan,
    canRemove: pin.canRemove,
    connections: pin.connections,
    input: pin.input,
    acceptedType: pin.acceptedType,
    typeState: pin.typeState,
    resolvedSchema: pin.resolvedSchema,
    status: pin.status,
  }) as PinData;
}
