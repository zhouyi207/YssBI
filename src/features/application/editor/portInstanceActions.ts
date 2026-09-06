import { executeGraphEdit } from "@/features/application/graphEditing";
import type { GraphEditOutcome } from "@/features/application/graphEditing/types";
import type { PortAddressDto } from "@/shared/types/domain/editorProjection";
import type { PortPlacementDto } from "@/shared/types/domain/editorMutation";

function isNonEmpty(value: string): boolean {
  return value.trim().length > 0;
}

export function addPortInstance(
  graphPath: string,
  nodeId: string,
  templateKey: string,
  placement: PortPlacementDto = { kind: "append" },
): Promise<GraphEditOutcome> {
  if (!isNonEmpty(nodeId) || !isNonEmpty(templateKey))
    return Promise.resolve({ status: "unavailable" });

  return executeGraphEdit(graphPath, "AddPortInstance", {
    nodeId,
    templateKey,
    placement,
  });
}

export function movePortInstance(
  graphPath: string,
  address: Extract<PortAddressDto, { kind: "instance" }>,
  placement: PortPlacementDto,
): Promise<GraphEditOutcome> {
  return executeGraphEdit(graphPath, "MovePortInstance", {
    address,
    placement,
  });
}

export function removePortInstance(
  graphPath: string,
  address: Extract<PortAddressDto, { kind: "instance" }>,
): Promise<GraphEditOutcome> {
  return executeGraphEdit(graphPath, "RemovePortInstance", {
    address,
  });
}
