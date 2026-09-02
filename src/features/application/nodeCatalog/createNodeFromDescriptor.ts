import {
  isNodeCreationDescriptor,
  type NodeCreationDescriptor,
} from "@/features/domain/nodeCatalog/creationDescriptor";
import type { NodePositionDto, PortAddressDto } from "@/shared/types/domain/editorProjection";
import {
  applyGraphDraftMutation,
  type ApplyGraphDraftMutationOutcome,
} from "@/features/application/graphDraft/graphDraftCoordinator";

export interface CreateNodeFromDescriptorInput {
  graphPath: string;
  locale: string;
  descriptor: NodeCreationDescriptor;
  position: NodePositionDto;
  connectFrom?: PortAddressDto | null;
}

export async function createNodeFromDescriptor(
  input: CreateNodeFromDescriptorInput,
): Promise<ApplyGraphDraftMutationOutcome> {
  if (!isNodeCreationDescriptor(input.descriptor)) {
    throw new Error("Unsupported node creation descriptor");
  }

  return applyGraphDraftMutation({
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
