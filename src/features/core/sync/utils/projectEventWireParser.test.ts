import { describe, expect, it } from 'vitest';
import editorProjection from '@/tests/fixtures/node-system-contracts/editor-projection.json';
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
const functionPath = 'functions/Sales Report 销售预测.yssbi-function';
const functionSignature = {
  parameters: [{ id: 'sales', name: 'Observed sales', type_name: 'Float64' }],
  return_type: 'Array<String>',
};
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

function functionResourceResult(functionRevision = 1) {
  const projection = structuredClone(editorProjection) as Record<string, unknown>;
  projection.graphPath = functionPath;
  projection.sourceRevision = 1;
  (projection.basis as Record<string, unknown>).graphPath = functionPath;
  (projection.basis as Record<string, unknown>).graphRevision = 1;
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
    projectionReplacements: [{
      graphPath: functionPath,
      projection,
      functionEditorProjection: {
        functionRevision,
        inputs: [{ id: 'sales', name: 'Observed sales', dataType: { kind: 'Float64' } }],
        outputs: [{
          id: 'return',
          name: 'Array<String>',
          dataType: { kind: 'Array', inner: { kind: 'String' } },
        }],
      },
    }],
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

  it('rejects removed worksheet side-channel and legacy document identity fields', () => {
    expect(() => parseResourceMutationCommittedPayload({
      result: { ...resourceResult, worksheetDeltas: [] },
    })).toThrow();

    for (const legacy of [{ id: 'legacy-id' }, { name: 'persisted document name' }]) {
      const result = worksheetResourceResult({
        kind: 'worksheet',
        patch: {
          before: worksheetDocumentState(),
          after: { ...worksheetDocumentState(), ...legacy },
        },
      });
      expect(() => parseResourceMutationCommittedPayload({ result })).toThrow();
    }
  });

  it('parses an exact function replacement with Rust-resolved editor pins', () => {
    const result = functionResourceResult();
    expect(parseResourceMutationCommittedPayload({ result })).toEqual({ result });
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
