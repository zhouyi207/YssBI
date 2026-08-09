import { describe, expect, it } from 'vitest';
import {
  parseGraphDeltaDto,
  parseGraphMutationResultDto,
  parseGraphProjectionReplacementDto,
} from './editorMutationWireParser';

const graphPath = 'events/Main.yssbi-event';
const functionPath = 'functions/Forecast.yssbi-function';
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

function projection(path: string, revision: number) {
  return {
    basis: {
      graphPath: path,
      graphRevision: revision,
      registryFingerprint: '0'.repeat(64),
      resourceVersions: {},
    },
    graphPath: path,
    sourceRevision: revision,
    nodes: [],
    connections: [],
    diagnostics: [],
    outcome: { type: 'success' },
    hasBlockingDiagnostics: false,
  };
}

function functionEditorProjection(revision = 5) {
  return {
    functionRevision: revision,
    inputs: [{ id: 'sales', name: 'Observed sales', dataType: { kind: 'Float64' } }],
    outputs: [{ id: 'return', name: 'Array<String>', dataType: { kind: 'Array', inner: { kind: 'String' } } }],
  };
}

function graphResult() {
  return {
    projectInstanceId: 'project-a',
    delta: delta(),
    projectionReplacement: {
      graphPath,
      projection: projection(graphPath, 5),
    },
    history: { canUndo: true, canRedo: false },
  };
}

describe('editor mutation wire parser', () => {
  it('strictly branches event and function projection replacement wire shapes', () => {
    const eventReplacement = { graphPath, projection: projection(graphPath, 5) };
    const functionReplacement = {
      graphPath: functionPath,
      projection: projection(functionPath, 5),
      functionEditorProjection: functionEditorProjection(),
    };

    expect(parseGraphProjectionReplacementDto(eventReplacement)).toEqual(eventReplacement);
    expect(parseGraphProjectionReplacementDto(functionReplacement)).toEqual(functionReplacement);
    expect(() => parseGraphProjectionReplacementDto({
      ...eventReplacement,
      functionEditorProjection: functionEditorProjection(),
    })).toThrow('projection replacement');
    expect(() => parseGraphProjectionReplacementDto({
      graphPath: functionPath,
      projection: projection(functionPath, 5),
    })).toThrow('projection replacement');
  });

  it.each([
    'events/Sales Report 中文.yssbi-event',
    'functions/销售 预测.yssbi-function',
  ])('parses opaque replacement path %j', (path) => {
    const replacement = path.startsWith('functions/')
      ? {
          graphPath: path,
          projection: projection(path, 5),
          functionEditorProjection: functionEditorProjection(),
        }
      : { graphPath: path, projection: projection(path, 5) };
    expect(parseGraphProjectionReplacementDto(replacement)).toEqual(replacement);
  });

  it('rejects empty and whitespace-only Struct keys in function replacement pins', () => {
    for (const inner of ['', '   ']) {
      const malformed = {
        graphPath: functionPath,
        projection: projection(functionPath, 5),
        functionEditorProjection: {
          ...functionEditorProjection(),
          outputs: [{ id: 'return', name: 'Model', dataType: { kind: 'Struct', inner } }],
        },
      };
      expect(() => parseGraphProjectionReplacementDto(malformed)).toThrow('projection replacement');
    }
  });

  it('parses every Rust graph patch operation from an exact delta', () => {
    expect(parseGraphDeltaDto(delta())).toEqual(delta());
  });

  it.each([
    'events/folder/sub-folder/Main.v2.yssbi-event',
    'functions/library/math/Calculate.yssbi-function',
    'events/Sales Report 中文.yssbi-event',
    'functions/销售 预测.yssbi-function',
  ])('accepts opaque graph mutation path %j', (nestedGraphPath) => {
    const nested = { ...delta(), graphPath: nestedGraphPath };
    expect(parseGraphDeltaDto(nested)).toEqual(nested);
  });

  it('rejects malformed graph delta identity, revisions, operations, and extra fields', () => {
    for (const malformedPath of [
      '',
      'not-a-resource',
      'events/Main.yssbi-function',
      'functions/Main.yssbi-event',
      'events//Main.yssbi-event',
      'events/../Main.yssbi-event',
    ]) {
      expect(() => parseGraphDeltaDto({ ...delta(), graphPath: malformedPath })).toThrow(
        'graphPath',
      );
    }
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
