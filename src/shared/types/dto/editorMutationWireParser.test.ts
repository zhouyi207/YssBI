import { describe, expect, it } from 'vitest';
import {
  parseGraphDeltaDto,
  parseGraphMutationResultDto,
} from './editorMutationWireParser';

const graphPath = 'events/Main.yssbi-event';
const operationId = '00000000-0000-0000-0000-000000000401';
const nodeId = '00000000-0000-0000-0000-000000000101';
const instanceId = '00000000-0000-0000-0000-000000000102';
const connectionId = '00000000-0000-0000-0000-000000000201';
const declaredAddress = { node_id: nodeId, port: { kind: 'declared', key: 'value' } };
const instanceAddress = {
  node_id: nodeId,
  port: { kind: 'instance', template: 'input', instance_id: instanceId },
};
const node = {
  id: nodeId,
  node_type: 'core.constant',
  position: { x: 1, y: 2 },
  parameters: {},
  user_label: null,
};
const connection = {
  id: connectionId,
  output: declaredAddress,
  input: instanceAddress,
  order: null,
};
const operations = [
  { operation: 'insert_node', node },
  { operation: 'remove_node', node },
  { operation: 'update_node', before: node, after: { ...node, user_label: 'After' } },
  {
    operation: 'insert_port_binding',
    address: instanceAddress,
    binding: { kind: 'user_created', order: 'a' },
  },
  {
    operation: 'remove_port_binding',
    address: instanceAddress,
    binding: {
      kind: 'orphan',
      origin: { kind: 'schema_field', source: 'databases/sales', field: 'amount' },
      order: 'b',
      last_known: { label: 'Amount' },
    },
  },
  { operation: 'insert_connection', connection },
  { operation: 'remove_connection', connection },
  {
    operation: 'set_input_state',
    address: instanceAddress,
    before: null,
    after: { literal_override: 12 },
  },
];

function delta() {
  return {
    graphPath,
    fromRevision: 4,
    toRevision: 5,
    causedBy: operationId,
    payload: { operations },
  };
}

function graphResult() {
  return {
    projectInstanceId: 'project-a',
    delta: delta(),
    projectionReplacement: {
      graphPath,
      projection: {
        basis: {
          graphPath,
          graphRevision: 5,
          registryFingerprint: '0'.repeat(64),
          resourceVersions: {},
        },
        graphPath,
        sourceRevision: 5,
        nodes: [],
        connections: [],
        diagnostics: [],
        hasBlockingDiagnostics: false,
      },
    },
    history: { canUndo: true, canRedo: false },
  };
}

describe('editor mutation wire parser', () => {
  it('parses every Rust graph patch operation from an exact delta', () => {
    expect(parseGraphDeltaDto(delta())).toEqual(delta());
  });

  it('rejects malformed graph delta identity, revisions, operations, and extra fields', () => {
    expect(() => parseGraphDeltaDto({ ...delta(), graphPath: '' })).toThrow('graphPath');
    expect(() => parseGraphDeltaDto({ ...delta(), toRevision: 5.5 })).toThrow('revision');
    expect(() => parseGraphDeltaDto({ ...delta(), causedBy: 'not-a-uuid' })).toThrow('causedBy');
    expect(() => parseGraphDeltaDto({ ...delta(), extra: true })).toThrow('exact');
    expect(() => parseGraphDeltaDto({
      ...delta(),
      payload: { operations: [{ operation: 'insert_node', node: { ...node, id: 'bad' } }] },
    })).toThrow('operation');
  });

  it('requires project identity and exact projection and history fields in graph results', () => {
    const result = graphResult();
    expect(parseGraphMutationResultDto(result).projectInstanceId).toBe('project-a');
    expect(() => parseGraphMutationResultDto({ ...result, projectInstanceId: undefined })).toThrow(
      'projectInstanceId',
    );
    expect(() => parseGraphMutationResultDto({ ...result, extra: true })).toThrow('exact');
    expect(() => parseGraphMutationResultDto({
      ...result,
      projectionReplacement: { ...result.projectionReplacement, extra: true },
    })).toThrow('projection');
    expect(() => parseGraphMutationResultDto({
      ...result,
      history: { ...result.history, extra: true },
    })).toThrow('history');
  });
});
