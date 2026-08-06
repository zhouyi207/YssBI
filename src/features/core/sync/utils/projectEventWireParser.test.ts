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
  toRevision: 2,
  causedBy: null,
  payload: { operations: [] },
};
const resourceResult = {
  operationId: '00000000-0000-0000-0000-000000000401',
  projectInstanceId,
  publicationRevision: 1,
  moves: [],
  deltas: [],
  projectionReplacements: [],
  projectionStatus: { status: 'complete', expectedGraphPaths: [] },
  history: { canUndo: false, canRedo: false },
};

describe('project event wire parser', () => {
  it('parses each complete Rust-generated project mutation event envelope', () => {
    expect(projectEvents.events.map(parseProjectMutationEvent)).toEqual(projectEvents.events);
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
    ['worksheet delta', {
      ...resourceResult,
      worksheetDeltas: [{
        id: 'worksheet-1',
        before: null,
        after: {
          schemaVersion: 1,
          revision: 1,
          id: 'worksheet-1',
          name: 'Worksheet',
          databaseId: 'database-1',
          chartType: 'line',
          encodings: {},
        },
        extra: true,
      }],
    }],
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
