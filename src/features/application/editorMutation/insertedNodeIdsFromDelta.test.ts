import { describe, expect, it } from 'vitest';
import type { GraphDeltaDto, GraphDocumentOperationDto } from '@/shared/types/domain/editorMutation';
import { insertedNodeIdsFromDelta } from './insertedNodeIdsFromDelta';

const node = (id: string) => ({
  id,
  node_type: 'tests.node',
  position: { x: 0, y: 0 },
  parameters: {},
  user_label: null,
});

function delta(operations: GraphDocumentOperationDto[]): GraphDeltaDto {
  return {
    graphPath: 'events/main.yssbi-event',
    fromRevision: 4,
    toRevision: 5,
    causedBy: 'operation-1',
    payload: { operations },
  };
}

describe('insertedNodeIdsFromDelta', () => {
  it('returns committed insert_node IDs in operation order', () => {
    expect(insertedNodeIdsFromDelta(delta([
      { operation: 'insert_node', node: node('node-b') },
      { operation: 'insert_node', node: node('node-a') },
    ]))).toEqual(['node-b', 'node-a']);
  });

  it('ignores every non-insert node, binding, input, and connection operation', () => {
    const address = { node_id: 'node-a', port: { kind: 'declared' as const, key: 'input' } };
    const connection = { id: 'connection-a', output: address, input: address, order: null };
    expect(insertedNodeIdsFromDelta(delta([
      { operation: 'remove_node', node: node('removed') },
      { operation: 'update_node', before: node('before'), after: node('after') },
      { operation: 'insert_port_binding', address, binding: { kind: 'user_created', order: 'a' } },
      { operation: 'remove_port_binding', address, binding: { kind: 'user_created', order: 'a' } },
      { operation: 'set_input_state', address, before: null, after: { literal_override: 1 } },
      { operation: 'insert_connection', connection },
      { operation: 'remove_connection', connection },
    ]))).toEqual([]);
  });
});
