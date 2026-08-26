import { describe, expect, it } from 'vitest';
import type { EditorGraphMutationDto } from './editorMutation';
import {
  parseEditorGraphMutationDto,
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
const projectedDeclaredAddress = { kind: 'declared' as const, nodeId, portKey: 'value' };
const phase1Mutations = [
  { type: 'deleteNodes', payload: { nodeIds: [nodeId] } },
  { type: 'disconnectConnections', payload: { connectionIds: [connectionId] } },
  { type: 'disconnectPort', payload: { address: projectedDeclaredAddress } },
  { type: 'disconnectNode', payload: { nodeId } },
  {
    type: 'moveConnections',
    payload: { source: projectedDeclaredAddress, target: projectedDeclaredAddress },
  },
] satisfies EditorGraphMutationDto[];
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
      last_known: { label: 'Amount', value_type: { Concrete: 'core.float64' } },
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

function projectionWithParameterEditor(): Record<string, unknown> {
  const value = projectionWithResolvedType(null);
  const projectedNode = (value.nodes as Array<Record<string, unknown>>)[0];
  projectedNode.parameterEditors = [{
    key: 'value',
    display: { title: 'Value', description: null },
    editor: 'number',
    presentation: 'inlineAndDetail',
    valueType: { kind: 'Int64' },
    multiline: false,
    value: 1,
    configuration: null,
    inheritedValue: null,
    valueSource: null,
    options: null,
  }];
  return value;
}

function projectionWithResolvedType(resolvedType: unknown): Record<string, unknown> {
  const value: Record<string, unknown> = projection(graphPath, 5);
  value.nodes = [{
    graphPath,
    sourceRevision: 5,
    nodeId,
    nodeTypeId: 'core.constant',
    position: { x: 1, y: 2 },
    display: {
      title: 'Constant',
      userLabel: null,
      iconId: null,
      styleId: null,
    },
    ports: [{
      address: { kind: 'declared', nodeId, portKey: 'value' },
      templateKey: 'value',
      display: { label: 'Value', instanceLabel: null },
      direction: 'output',
      kind: 'data',
      instanceKind: 'declared',
      orphan: false,
      canRemove: false,
      connections: {
        current: 0,
        maximum: null,
        ordered: false,
        canAppend: true,
        canReplace: false,
        canMove: false,
      },
      input: null,
      resolvedType,
      resolvedSchema: null,
      status: 'resolved',
    }],
    parameterEditors: [],
    capabilities: {
      managed: false,
      canCopy: true,
      canDelete: true,
      canEditLabel: true,
      canEditParameters: false,
      hasDynamicPorts: false,
      supportsInlineLiterals: false,
    },
    diagnostics: [],
  }];
  return value;
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
  it('parses the exact InsertReroute DTO wire shape', () => {
    const mutation = {
      type: 'insertReroute',
      payload: {
        connectionId: 'edge-1',
        position: { x: 120, y: 80 },
      },
    };

    expect(parseEditorGraphMutationDto(mutation)).toEqual(mutation);
  });

  it.each([
    { type: 'insertReroute', payload: { connectionId: '', position: { x: 120, y: 80 } } },
    { type: 'insertReroute', payload: { connectionId: '   ', position: { x: 120, y: 80 } } },
    { type: 'unknownReroute', payload: { connectionId: 'edge-1', position: { x: 120, y: 80 } } },
    { type: 'insertReroute', payload: { connectionId: 'edge-1', position: { x: Infinity, y: 80 } } },
    { type: 'insertReroute', payload: { connectionId: 'edge-1', position: { x: -Infinity, y: 80 } } },
    { type: 'insertReroute', payload: { connectionId: 'edge-1', position: { x: 120, y: NaN } } },
    { type: 'insertReroute', payload: { connectionId: 'edge-1', position: { x: '120', y: 80 } } },
    { type: 'insertReroute', payload: { connectionId: 1, position: { x: 120, y: 80 } } },
    { type: 'insertReroute', payload: { connectionId: 'edge-1', position: { x: 120 } } },
    {
      type: 'insertReroute',
      payload: { connectionId: 'edge-1', position: { x: 120, y: 80, z: 0 } },
    },
    {
      type: 'insertReroute',
      payload: { connectionId: 'edge-1', position: { x: 120, y: 80 }, extra: true },
    },
    {
      type: 'insertReroute',
      payload: { connectionId: 'edge-1', position: { x: 120, y: 80 } },
      extra: true,
    },
  ])('rejects malformed InsertReroute DTO wire shape %#', (mutation) => {
    expect(() => parseEditorGraphMutationDto(mutation)).toThrow('InsertReroute');
  });

  it('exposes all Phase 1 collection and connection intent DTO variants', () => {
    expect(phase1Mutations.map((mutation) => mutation.type)).toEqual([
      'deleteNodes',
      'disconnectConnections',
      'disconnectPort',
      'disconnectNode',
      'moveConnections',
    ]);
  });

  it('requires all six exact connection capability fields', () => {
    const valid = projectionWithResolvedType({
      display: 'Float64',
      resolved: true,
      dataType: { kind: 'Float64' },
    });
    const node = (valid.nodes as Array<Record<string, unknown>>)[0];
    const port = (node.ports as Array<Record<string, unknown>>)[0];
    const capability = port.connections as Record<string, unknown>;

    expect(parseGraphProjectionReplacementDto({ graphPath, projection: valid }))
      .toEqual({ graphPath, projection: valid });

    for (const key of ['current', 'maximum', 'ordered', 'canAppend', 'canReplace', 'canMove']) {
      const malformed = structuredClone(valid);
      const malformedNode = (malformed.nodes as Array<Record<string, unknown>>)[0];
      const malformedPort = (malformedNode.ports as Array<Record<string, unknown>>)[0];
      delete (malformedPort.connections as Record<string, unknown>)[key];
      expect(() => parseGraphProjectionReplacementDto({ graphPath, projection: malformed }))
        .toThrow('projection replacement');
    }

    for (const key of ['canAppend', 'canReplace', 'canMove']) {
      const malformed = structuredClone(valid);
      const malformedNode = (malformed.nodes as Array<Record<string, unknown>>)[0];
      const malformedPort = (malformedNode.ports as Array<Record<string, unknown>>)[0];
      (malformedPort.connections as Record<string, unknown>)[key] = 'yes';
      expect(() => parseGraphProjectionReplacementDto({ graphPath, projection: malformed }))
        .toThrow('projection replacement');
    }

    capability.extra = false;
    expect(() => parseGraphProjectionReplacementDto({ graphPath, projection: valid }))
      .toThrow('projection replacement');
  });

  it('requires strict parameter editor presentation metadata', () => {
    const replacement = {
      graphPath,
      projection: projectionWithParameterEditor(),
    };
    expect(parseGraphProjectionReplacementDto(replacement)).toEqual(replacement);

    const missing = structuredClone(replacement) as any;
    delete missing.projection.nodes[0].parameterEditors[0].presentation;
    expect(() => parseGraphProjectionReplacementDto(missing)).toThrow('projection replacement');

    const invalid = structuredClone(replacement) as any;
    invalid.projection.nodes[0].parameterEditors[0].presentation = 'inlineOnly';
    expect(() => parseGraphProjectionReplacementDto(invalid)).toThrow('projection replacement');
  });

  it.each([
    ['missing inheritedValue', (editor: Record<string, unknown>) => { delete editor.inheritedValue; }],
    ['missing valueSource', (editor: Record<string, unknown>) => { delete editor.valueSource; }],
    ['missing options', (editor: Record<string, unknown>) => { delete editor.options; }],
    ['invalid valueSource casing', (editor: Record<string, unknown>) => {
      editor.valueSource = 'Project';
    }],
    ['non-string options', (editor: Record<string, unknown>) => { editor.options = [1]; }],
    ['missing valueType', (editor: Record<string, unknown>) => { delete editor.valueType; }],
    ['string valueType', (editor: Record<string, unknown>) => { editor.valueType = 'Int64'; }],
    ['malformed valueType', (editor: Record<string, unknown>) => {
      editor.valueType = { kind: 'Array' };
    }],
    ['valueType with an extra key', (editor: Record<string, unknown>) => {
      editor.valueType = { kind: 'Int64', extra: true };
    }],
    ['parameter editor with an extra key', (editor: Record<string, unknown>) => {
      editor.extra = true;
    }],
  ])('rejects parameter editor %s', (_, mutate) => {
    const replacement = {
      graphPath,
      projection: projectionWithParameterEditor(),
    };
    const projectedNode = (replacement.projection.nodes as Array<Record<string, unknown>>)[0];
    const editor = (projectedNode.parameterEditors as Array<Record<string, unknown>>)[0];
    mutate(editor);
    expect(() => parseGraphProjectionReplacementDto(replacement)).toThrow('projection replacement');
  });

  it('requires an exact structured dataType on every non-null resolved type', () => {
    const malformedResolvedTypes = [
      { display: 'Float64', resolved: true },
      { display: 'Float64', resolved: true, dataType: 'Float64' },
      { display: 'Float64', resolved: true, dataType: { kind: 'Float32' } },
      { display: 'Array', resolved: true, dataType: { kind: 'Array' } },
      { display: 'Float64', resolved: true, dataType: { kind: 'Float64', extra: true } },
    ];

    for (const resolvedType of malformedResolvedTypes) {
      expect(() => parseGraphProjectionReplacementDto({
        graphPath,
        projection: projectionWithResolvedType(resolvedType),
      })).toThrow('projection replacement');
    }
  });

  it('accepts structured dataType independently from its display label', () => {
    const replacement = {
      graphPath,
      projection: projectionWithResolvedType({
        display: 'Not parsed by the frontend',
        resolved: true,
        dataType: { kind: 'DataSeries', inner: { kind: 'Float64' } },
      }),
    };

    expect(parseGraphProjectionReplacementDto(replacement)).toEqual(replacement);
  });

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

  it('accepts and carries every structured orphan value_type wire variant', () => {
    const valueTypes = [
      { Concrete: 'core.int64' },
      { Generic: 'value' },
      { Applied: { constructor: 'core.array', arguments: [{ Concrete: 'core.string' }] } },
      { Union: [{ Concrete: 'core.int64' }, 'Unknown'] },
      'Unknown',
    ];

    for (const value_type of valueTypes) {
      const value = delta();
      const operation = structuredClone(operations[4]) as Record<string, any>;
      operation.binding.last_known.value_type = value_type;
      (value.payload.operations as unknown[]) = [operation];

      expect(parseGraphDeltaDto(value)).toEqual(value);
    }
  });

  it('accepts resolved metadata with an optional current value type', () => {
    for (const last_known of [
      { label: 'Amount', value_type: { Concrete: 'core.float64' } },
      { label: 'Amount' },
    ]) {
      const value = delta();
      const operation = structuredClone(operations[4]) as Record<string, any>;
      operation.binding = {
        kind: 'resolved',
        origin: operation.binding.origin,
        order: operation.binding.order,
        last_known,
      };
      (value.payload.operations as unknown[]) = [operation];

      expect(parseGraphDeltaDto(value)).toEqual(value);
    }
  });

  it('rejects a resolved binding without last_known metadata', () => {
    const value = delta();
    const operation = structuredClone(operations[4]) as Record<string, any>;
    operation.binding = {
      kind: 'resolved',
      origin: operation.binding.origin,
      order: operation.binding.order,
    };
    (value.payload.operations as unknown[]) = [operation];

    expect(() => parseGraphDeltaDto(value)).toThrow('graph patch operation');
  });

  it('rejects malformed or extended resolved last_known metadata', () => {
    for (const last_known of [
      null,
      {},
      { label: 42 },
      { label: 'Amount', value_type: { Concrete: 42 } },
      { label: 'Amount', extra: true },
    ]) {
      const value = delta();
      const operation = structuredClone(operations[4]) as Record<string, any>;
      operation.binding = {
        kind: 'resolved',
        origin: operation.binding.origin,
        order: operation.binding.order,
        last_known,
      };
      (value.payload.operations as unknown[]) = [operation];

      expect(() => parseGraphDeltaDto(value)).toThrow('graph patch operation');
    }
  });

  it('keeps label-only orphan metadata without inferring value_type', () => {
    const value = delta();
    const operation = structuredClone(operations[4]) as Record<string, any>;
    delete operation.binding.last_known.value_type;
    (value.payload.operations as unknown[]) = [operation];

    const parsed = parseGraphDeltaDto(value);
    expect(parsed).toEqual(value);
    expect((parsed.payload.operations[0] as Record<string, any>)
      .binding.last_known).not.toHaveProperty('value_type');
  });

  it('rejects malformed, inferred, or extended orphan value_type wire shapes', () => {
    const malformed = [
      null,
      'core.int64',
      { Concrete: 'core.int64', extra: true },
      { Applied: { constructor: 'core.array', arguments: 'core.string' } },
      { Union: 'core.int64' },
      { Unknown: true },
    ];

    for (const value_type of malformed) {
      const value = delta();
      const operation = structuredClone(operations[4]) as Record<string, any>;
      operation.binding.last_known.value_type = value_type;
      (value.payload.operations as unknown[]) = [operation];

      expect(() => parseGraphDeltaDto(value)).toThrow('graph patch operation');
    }
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

  it('accepts only empty operations for a same-revision graph delta', () => {
    const noop = { ...delta(), toRevision: 4, payload: { operations: [] } };

    expect(parseGraphDeltaDto(noop)).toEqual(noop);
    expect(() => parseGraphDeltaDto({
      ...noop,
      payload: { operations: [operations[0]] },
    })).toThrow('revision');
    expect(() => parseGraphDeltaDto({
      ...noop,
      toRevision: 5,
    })).toThrow('revision');
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
    expect(parseGraphMutationResultDto(result, 'project-a').projectInstanceId).toBe('project-a');
    expect(() => parseGraphMutationResultDto(result, 'project-b')).toThrow('projectInstanceId');
    expect(() => parseGraphMutationResultDto(
      { ...result, projectInstanceId: undefined },
      'project-a',
    )).toThrow('projectInstanceId');
    expect(() => parseGraphMutationResultDto({ ...result, extra: true }, 'project-a'))
      .toThrow('exact');
    expect(() => parseGraphMutationResultDto({
      ...result,
      projectionReplacement: { ...result.projectionReplacement, extra: true },
    }, 'project-a')).toThrow('projection');
    expect(() => parseGraphMutationResultDto({
      ...result,
      history: { ...result.history, extra: true },
    }, 'project-a')).toThrow('history');
  });
});
