import { describe, expect, it } from 'vitest';
import editorProjection from '@/tests/fixtures/node-system-contracts/editor-projection.json';
import functionEditorProjection from '@/tests/fixtures/node-system-contracts/function-editor-projection.json';
import projectEvents from '@/tests/fixtures/node-system-contracts/project-events.json';
import {
  parseGraphDeltaEventPayload,
  parseProjectMutationEvent,
  parseResourceMutationCommittedPayload,
} from './projectEventWireParser';

const projectInstanceId = '00000000-0000-0000-0000-000000000601';
const delta = {
  graphPath: 'events/Main.yssbi-event',
  fromRevision: 1,
  toRevision: 1,
  causedBy: null,
  payload: { operations: [] },
};
const operationId = '00000000-0000-0000-0000-000000000401';
const nodeId = '00000000-0000-0000-0000-000000000101';
const instanceId = '00000000-0000-0000-0000-000000000102';
const functionPath = functionEditorProjection.replacement.graphPath;
const functionSignature = functionEditorProjection.indexRow.functionSignature;
const resourceResult = {
  operationId,
  projectInstanceId,
  publicationRevision: 1,
  moves: [],
  deltas: [],
  projectionReplacements: [],
  projectionStatus: { status: 'complete', expectedGraphPaths: [] },
  history: { canUndo: false, canRedo: false },
};

function worksheetDocumentState() {
  return {
    databaseId: 'database-1',
    chartType: 'scatter',
    encodings: { x: 'region', y: 'revenue' },
  };
}

function worksheetResourceResult(
  payload: Record<string, unknown>,
  resourceKey = 'opaque worksheet / 路径',
  fromRevision = 0,
  toRevision = 1,
) {
  return {
    ...resourceResult,
    deltas: [{
      resource: { kind: 'worksheet', key: resourceKey },
      fromRevision,
      toRevision,
      causedBy: operationId,
      payload,
    }],
  };
}

function graphResourceResult(value_type: unknown) {
  return {
    ...resourceResult,
    projectionStatus: {
      status: 'incomplete',
      invalidatedGraphPaths: ['events/Main.yssbi-event'],
    },
    deltas: [{
      resource: { kind: 'graph', key: 'events/Main.yssbi-event' },
      fromRevision: 0,
      toRevision: 1,
      causedBy: operationId,
      payload: {
        kind: 'graph',
        patch: {
          operations: [{
            operation: 'insert_port_binding',
            address: {
              node_id: nodeId,
              port: { kind: 'instance', template: 'columns', instance_id: instanceId },
            },
            binding: {
              kind: 'orphan',
              origin: { kind: 'schema_field', source: 'databases/main', field: 'amount' },
              order: 'a',
              last_known: { label: 'Amount', value_type },
            },
          }],
        },
      },
    }],
  };
}

function functionResourceResult(functionRevision = 1) {
  const replacement = structuredClone(functionEditorProjection.replacement);
  replacement.functionEditorProjection.functionRevision = functionRevision;
  return {
    ...resourceResult,
    deltas: [{
      resource: { kind: 'function', key: functionPath },
      fromRevision: 0,
      toRevision: 1,
      causedBy: operationId,
      payload: {
        kind: 'function',
        patch: {
          before: { parameters: [], return_type: null },
          after: functionSignature,
        },
      },
    }],
    projectionReplacements: [replacement],
    projectionStatus: { status: 'complete', expectedGraphPaths: [functionPath] },
  };
}

describe('project event wire parser', () => {
  it('parses every semantic Rust-generated worksheet direct result and event envelope', () => {
    expect(projectEvents.resourceMutationResults.map(({ scenario, result }) => ({
      scenario,
      parsed: parseResourceMutationCommittedPayload({ result }).result,
    }))).toEqual(projectEvents.resourceMutationResults.map(({ scenario, result }) => ({
      scenario,
      parsed: result,
    })));
    expect(projectEvents.events.map(parseProjectMutationEvent)).toEqual(projectEvents.events);
    expect(projectEvents.resourceMutationResults.map(({ scenario }) => scenario)).toEqual([
      'create',
      'save',
      'rename',
      'remove',
      'undo',
      'redo',
    ]);
    expect(projectEvents.events.slice(1).map((event) => event.payload.payload.result))
      .toEqual(projectEvents.resourceMutationResults.map(({ result }) => result));
  });

  it('rejects extra and unknown outer or inner project event envelope fields', () => {
    const valid = projectEvents.events[0];
    expect(() => parseProjectMutationEvent({ ...valid, extra: true })).toThrow();
    expect(() => parseProjectMutationEvent({ ...valid, type: 'Legacy' })).toThrow();
    expect(() => parseProjectMutationEvent({
      ...valid,
      payload: { ...valid.payload, extra: true },
    })).toThrow();
    expect(() => parseProjectMutationEvent({
      ...valid,
      payload: { ...valid.payload, type: 'Legacy' },
    })).toThrow();
  });

  it('parses the exact Rust GraphDelta payload', () => {
    expect(parseGraphDeltaEventPayload({ projectInstanceId, delta })).toEqual({
      projectInstanceId,
      delta,
    });
  });

  it('rejects GraphDelta payloads with missing identity or extra envelope fields', () => {
    expect(() => parseGraphDeltaEventPayload({ delta })).toThrow('projectInstanceId');
    expect(() => parseGraphDeltaEventPayload({ projectInstanceId, delta, extra: true })).toThrow(
      'exact',
    );
  });

  it.each([
    ['document', worksheetResourceResult({
      kind: 'worksheet',
      patch: { before: worksheetDocumentState(), after: worksheetDocumentState() },
    })],
    ['lifecycle', worksheetResourceResult({
      kind: 'resource_lifecycle',
      patch: {
        before: null,
        after: {
          revision: 0,
          path: 'worksheets/Sales Report.yssbi-worksheet',
          kind: 'worksheet',
          name: 'Sales Report',
        },
      },
    }, 'worksheets/Sales Report.yssbi-worksheet', 0, 0)],
    ['move', {
      ...worksheetResourceResult({
        kind: 'resource_move',
        patch: { from: 'opaque source', to: 'opaque destination' },
      }, 'opaque destination'),
      moves: [{
        from: 'opaque source',
        to: 'opaque destination',
        kind: 'worksheet',
        name: 'Destination',
      }],
    }],
  ])('parses an exact canonical worksheet %s envelope', (_kind, result) => {
    expect(parseResourceMutationCommittedPayload({ result })).toEqual({ result });
  });

  it('parses an exact function replacement with Rust-resolved editor pins', () => {
    const result = functionResourceResult();
    expect(parseResourceMutationCommittedPayload({ result })).toEqual({ result });
  });

  it('accepts and carries structured orphan value_type in resource graph patches', () => {
    const valueTypes = [
      { Concrete: 'core.float64' },
      { Applied: { constructor: 'core.array', arguments: [{ Generic: 'element' }] } },
      { Union: [{ Concrete: 'core.int64' }, 'Unknown'] },
      'Unknown',
    ];

    for (const value_type of valueTypes) {
      const result = graphResourceResult(value_type);
      expect(parseResourceMutationCommittedPayload({ result })).toEqual({ result });
    }
  });

  it('rejects malformed or extended orphan value_type in resource graph patches', () => {
    const malformed = [
      null,
      'core.float64',
      { Concrete: 'core.float64', extra: true },
      { Applied: { constructor: 'core.array', arguments: [] }, extra: true },
      { Union: 'core.float64' },
    ];

    for (const value_type of malformed) {
      expect(() => parseResourceMutationCommittedPayload({
        result: graphResourceResult(value_type),
      })).toThrow('resource deltas');
    }
  });

  it('accepts resolved last_known metadata with an optional current value type', () => {
    for (const last_known of [
      { label: 'Amount', value_type: { Concrete: 'core.float64' } },
      { label: 'Amount' },
    ]) {
      const result = graphResourceResult({ Concrete: 'core.float64' }) as Record<string, any>;
      const binding = result.deltas[0].payload.patch.operations[0].binding;
      result.deltas[0].payload.patch.operations[0].binding = {
        kind: 'resolved',
        origin: binding.origin,
        order: binding.order,
        last_known,
      };

      expect(parseResourceMutationCommittedPayload({ result })).toEqual({ result });
    }
  });

  it('rejects a resolved binding without last_known metadata', () => {
    const result = graphResourceResult({ Concrete: 'core.float64' }) as Record<string, any>;
    const binding = result.deltas[0].payload.patch.operations[0].binding;
    result.deltas[0].payload.patch.operations[0].binding = {
      kind: 'resolved',
      origin: binding.origin,
      order: binding.order,
    };

    expect(() => parseResourceMutationCommittedPayload({ result })).toThrow('resource deltas');
  });

  it('rejects malformed resolved last_known metadata', () => {
    const result = graphResourceResult({ Concrete: 'core.float64' }) as Record<string, any>;
    const binding = result.deltas[0].payload.patch.operations[0].binding;
    result.deltas[0].payload.patch.operations[0].binding = {
      kind: 'resolved',
      origin: binding.origin,
      order: binding.order,
      last_known: { label: 'Amount', value_type: { Concrete: 42 } },
    };

    expect(() => parseResourceMutationCommittedPayload({ result })).toThrow();
  });

  it('accepts label-only orphan metadata without inferring value_type', () => {
    const result = graphResourceResult({ Concrete: 'core.float64' }) as Record<string, any>;
    delete result.deltas[0].payload.patch.operations[0].binding.last_known.value_type;

    const parsed = parseResourceMutationCommittedPayload({ result });
    expect(parsed).toEqual({ result });
    expect((parsed.result.deltas[0].payload as Record<string, any>)
      .patch.operations[0].binding.last_known).not.toHaveProperty('value_type');
  });

  it('rejects a function replacement revision that disagrees with its function delta', () => {
    expect(() => parseResourceMutationCommittedPayload({
      result: functionResourceResult(2),
    })).toThrow('function delta');
  });

  it('matches function revision to the function delta even when a graph delta comes first', () => {
    const result = functionResourceResult(2);
    const deltas = result.deltas as unknown[];
    deltas.unshift({
      resource: { kind: 'graph', key: functionPath },
      fromRevision: 0,
      toRevision: 1,
      causedBy: operationId,
      payload: { kind: 'graph', patch: { operations: [] } },
    });

    expect(() => parseResourceMutationCommittedPayload({ result })).toThrow('function delta');
  });

  it('parses only an exact valid ResourceMutationCommitted payload', () => {
    const parsed = parseResourceMutationCommittedPayload({ result: resourceResult });
    expect(parsed).toEqual({ result: resourceResult });
    expect(parsed.result).not.toBe(resourceResult);
    expect(parsed.result.history).not.toBe(resourceResult.history);
    expect(() => parseResourceMutationCommittedPayload({ result: resourceResult, extra: true }))
      .toThrow('exact');
    expect(() => parseResourceMutationCommittedPayload({
      result: { ...resourceResult, publicationRevision: 0 },
    })).toThrow('publication revision');
  });

  it.each([
    ['result', { ...resourceResult, extra: true }],
    ['move', {
      ...resourceResult,
      moves: [{
        from: 'events/Before.yssbi-event',
        to: 'events/After.yssbi-event',
        kind: 'event',
        name: 'After',
        extra: true,
      }],
    }],
    ['history', { ...resourceResult, history: { ...resourceResult.history, extra: true } }],
    ['projection status', {
      ...resourceResult,
      projectionStatus: { ...resourceResult.projectionStatus, extra: true },
    }],
    ['worksheet document', worksheetResourceResult({
      kind: 'worksheet',
      patch: {
        before: worksheetDocumentState(),
        after: { ...worksheetDocumentState(), extra: true },
      },
    })],
    ['projection replacement', {
      ...resourceResult,
      projectionReplacements: [{
        graphPath: editorProjection.graphPath,
        projection: editorProjection,
        extra: true,
      }],
      projectionStatus: {
        status: 'complete',
        expectedGraphPaths: [editorProjection.graphPath],
      },
    }],
    ['resource delta resource', {
      ...resourceResult,
      deltas: [{
        resource: { kind: 'graph', key: 'events/Main.yssbi-event', extra: true },
        fromRevision: 1,
        toRevision: 2,
        causedBy: null,
        payload: { kind: 'graph', patch: { operations: [] } },
      }],
      projectionStatus: {
        status: 'incomplete',
        invalidatedGraphPaths: ['events/Main.yssbi-event'],
      },
    }],
    ['resource delta payload', {
      ...resourceResult,
      deltas: [{
        resource: { kind: 'graph', key: 'events/Main.yssbi-event' },
        fromRevision: 1,
        toRevision: 2,
        causedBy: null,
        payload: { kind: 'graph', patch: { operations: [] }, extra: true },
      }],
      projectionStatus: {
        status: 'incomplete',
        invalidatedGraphPaths: ['events/Main.yssbi-event'],
      },
    }],
    ['graph operation node', {
      ...resourceResult,
      deltas: [{
        resource: { kind: 'graph', key: 'events/Main.yssbi-event' },
        fromRevision: 1,
        toRevision: 2,
        causedBy: null,
        payload: {
          kind: 'graph',
          patch: {
            operations: [{
              operation: 'insert_node',
              node: {
                id: '00000000-0000-0000-0000-000000000001',
                node_type: 'yssbi.constant.bool',
                position: { x: 0, y: 0 },
                parameters: {},
                user_label: null,
                extra: true,
              },
            }],
          },
        },
      }],
      projectionStatus: {
        status: 'incomplete',
        invalidatedGraphPaths: ['events/Main.yssbi-event'],
      },
    }],
  ])('rejects an extra key in nested %s', (_label, result) => {
    expect(() => parseResourceMutationCommittedPayload({ result })).toThrow();
  });

  it('rejects unknown nested variants', () => {
    expect(() => parseResourceMutationCommittedPayload({
      result: {
        ...resourceResult,
        projectionStatus: { status: 'legacy', expectedGraphPaths: [] },
      },
    })).toThrow();
    expect(() => parseResourceMutationCommittedPayload({
      result: {
        ...resourceResult,
        deltas: [{
          resource: { kind: 'graph', key: 'events/Main.yssbi-event' },
          fromRevision: 1,
          toRevision: 2,
          causedBy: null,
          payload: { kind: 'legacy', patch: {} },
        }],
      },
    })).toThrow();
  });
});
