import { describe, expect, it } from 'vitest';
import type { ResourceDeltaDto } from '@/shared/types/dto/editorMutation';
import { areResourceDeltasValid } from './resourceMutationWireValidator';

const operationId = '00000000-0000-0000-0000-000000000401';

function databaseDecl(name: string) {
  return {
    id: 'sales',
    engine: { duckDb: { path: 'database/project.duckdb', table: 'sales' } },
    schemaVersion: 1,
    required: false,
    name,
  };
}

function databaseDelta(): ResourceDeltaDto {
  return {
    resource: { kind: 'database', key: 'opaque database / 路径' },
    fromRevision: 4,
    toRevision: 5,
    causedBy: operationId,
    payload: {
      kind: 'database',
      patch: { before: databaseDecl('Before'), after: databaseDecl('After') },
    },
  };
}

function graphDelta(): ResourceDeltaDto {
  return {
    resource: { kind: 'graph', key: 'events/Main.yssbi-event' },
    fromRevision: 4,
    toRevision: 5,
    causedBy: operationId,
    payload: { kind: 'graph', patch: { operations: [] } },
  };
}

function functionDelta(): ResourceDeltaDto {
  return {
    resource: { kind: 'function', key: 'functions/library/math/Calculate.yssbi-function' },
    fromRevision: 4,
    toRevision: 5,
    causedBy: operationId,
    payload: {
      kind: 'function',
      patch: {
        before: { parameters: [], return_type: null },
        after: { parameters: [], return_type: null },
      },
    },
  };
}

describe('resource mutation wire envelope', () => {
  it.each([
    ['database', databaseDelta],
    ['graph', graphDelta],
  ])('accepts a canonical %s delta', (_kind, createDelta) => {
    expect(areResourceDeltasValid([createDelta()])).toBe(true);
  });

  it.each([
    ['database', databaseDelta],
    ['graph', graphDelta],
  ])('rejects an extra top-level field on a %s delta', (_kind, createDelta) => {
    const delta = { ...createDelta(), unexpected: true };

    expect(areResourceDeltasValid([delta])).toBe(false);
  });

  it.each([
    ['resource', 'resource'],
    ['fromRevision', 'fromRevision'],
    ['toRevision', 'toRevision'],
    ['causedBy', 'causedBy'],
    ['payload', 'payload'],
  ])('rejects a delta missing the required %s field', (_case, key) => {
    const delta = structuredClone(databaseDelta()) as unknown as Record<string, unknown>;
    delete delta[key];

    expect(areResourceDeltasValid([delta])).toBe(false);
  });

  it('preserves the graph branch acceptance of legal nested extension fields', () => {
    const delta = structuredClone(graphDelta()) as unknown as Record<string, any>;
    delta.resource.extension = true;
    delta.payload.extension = true;
    delta.payload.patch.extension = true;

    expect(areResourceDeltasValid([delta])).toBe(true);
  });

  it.each([
    'events/Main.yssbi-function',
    'events/../Main.yssbi-event',
    'events//Main.yssbi-event',
  ])('rejects malformed graph resource identity %j', (key) => {
    const delta = graphDelta();
    delta.resource.key = key;

    expect(areResourceDeltasValid([delta])).toBe(false);
  });

  it('accepts nested and opaque event and function resource identities', () => {
    const event = graphDelta();
    event.resource.key = 'events/Sales Report 中文.yssbi-event';
    const functionResource = functionDelta();
    functionResource.resource.key = 'functions/销售 预测.yssbi-function';

    expect(areResourceDeltasValid([event])).toBe(true);
    expect(areResourceDeltasValid([functionResource])).toBe(true);
  });
});

describe('database resource mutation wire', () => {
  it('accepts the exact canonical database resource and document patch shape with an opaque key', () => {
    expect(areResourceDeltasValid([databaseDelta()])).toBe(true);
  });

  it.each([
    ['empty opaque resource key', (delta: Record<string, any>) => { delta.resource.key = ''; }],
    ['resource extra field', (delta: Record<string, any>) => { delta.resource.extra = true; }],
    ['mismatched payload kind', (delta: Record<string, any>) => { delta.payload.kind = 'variable'; }],
    ['patch extra field', (delta: Record<string, any>) => { delta.payload.patch.extra = true; }],
    ['missing declaration ID', (delta: Record<string, any>) => { delete delta.payload.patch.after.id; }],
    ['malformed engine', (delta: Record<string, any>) => { delta.payload.patch.after.engine = { duckDb: { path: 4, table: 'sales' } }; }],
    ['two absent declarations', (delta: Record<string, any>) => {
      delta.payload.patch.before = null;
      delta.payload.patch.after = null;
    }],
    ['different declaration IDs', (delta: Record<string, any>) => { delta.payload.patch.after.id = 'other'; }],
  ])('rejects %s', (_case, mutate) => {
    const delta = structuredClone(databaseDelta()) as unknown as Record<string, any>;
    mutate(delta);
    expect(areResourceDeltasValid([delta])).toBe(false);
  });
});
