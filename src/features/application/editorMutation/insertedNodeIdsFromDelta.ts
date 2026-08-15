import type { GraphDeltaDto } from '@/shared/types/dto/editorMutation';

export function insertedNodeIdsFromDelta(delta: GraphDeltaDto): string[] {
  return delta.payload.operations.flatMap((operation) => (
    operation.operation === 'insert_node' ? [operation.node.id] : []
  ));
}
