import {
  isNodeCreationDescriptor,
  type NodeCreationDescriptor,
} from "@/features/domain/nodeCatalog/creationDescriptor";
import type { NodePositionDto, PortAddressDto } from "@/shared/types/domain/editorProjection";
import {
  applyGraphMutation,
  type ApplyGraphMutationOutcome,
} from "@/features/application/graphEditing/graphEditCoordinator";

export interface CreateNodeFromDescriptorInput {
  graphPath: string;
  locale: string;
  descriptor: NodeCreationDescriptor;
  position: NodePositionDto;
  connectFrom?: PortAddressDto | null;
}

export async function createNodeFromDescriptor(
  input: CreateNodeFromDescriptorInput,
): Promise<ApplyGraphMutationOutcome> {
  if (!isNodeCreationDescriptor(input.descriptor)) {
    throw new Error("Unsupported node creation descriptor");
  }

  return applyGraphMutation({
    graphPath: input.graphPath,
    locale: input.locale,
    mutation: {
      type: "createNode",
      payload: {
        descriptor: input.descriptor,
        position: input.position,
        userLabel: null,
        connectFrom: input.connectFrom ?? null,
      },
    },
  });
}
