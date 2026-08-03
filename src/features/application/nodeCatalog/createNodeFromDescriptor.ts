import {
  isNodeCreationDescriptor,
  type NodeCreationDescriptor,
} from '@/features/domain/nodeCatalog/creationDescriptor';
import type { NodePositionDto } from '@/shared/types/dto/editorProjection';
import {
  executeEditorMutation,
  type ExecuteEditorMutationOutcome,
} from '@/features/application/editorMutation/editorMutationCoordinator';

export interface CreateNodeFromDescriptorInput {
  graphPath: string;
  locale: string;
  descriptor: NodeCreationDescriptor;
  position: NodePositionDto;
}

export async function createNodeFromDescriptor(
  input: CreateNodeFromDescriptorInput,
): Promise<ExecuteEditorMutationOutcome> {
  if (!isNodeCreationDescriptor(input.descriptor)) {
    throw new Error('Unsupported node creation descriptor');
  }

  return executeEditorMutation({
    graphPath: input.graphPath,
    locale: input.locale,
    mutation: {
      type: 'createNode',
      payload: {
        descriptor: input.descriptor,
        position: input.position,
        userLabel: null,
      },
    },
  });
}
