import { describe, expect, it } from 'vitest';
import type { ResourceDeltaDto } from '@/shared/types/dto/editorMutation';
import { areResourceDeltasValid } from '@/shared/types/dto/resourceMutationWireValidator';

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

function worksheetDocumentState(
  databaseId: string,
  chartType: 'histogram' | 'scatter' | 'line',
) {
  return { databaseId, chartType, encodings: { x: 'region', y: 'revenue' } };
}

function worksheetDelta(): ResourceDeltaDto {
  return {
    resource: { kind: 'worksheet', key: 'opaque worksheet / 路径' },
    fromRevision: 4,
    toRevision: 5,
    causedBy: operationId,
    payload: {
      kind: 'worksheet',
      patch: {
        before: worksheetDocumentState('database-before', 'histogram'),
        after: worksheetDocumentState('database-after', 'scatter'),
      },
    },
  };
}

function worksheetLifecycleDelta(): ResourceDeltaDto {
  return {
    resource: { kind: 'worksheet', key: 'worksheets/Sales Report.yssbi-worksheet' },
    fromRevision: 0,
    toRevision: 0,
    causedBy: operationId,
    payload: {
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
    },
  };
}

function worksheetMoveDelta(): ResourceDeltaDto {
  return {
    resource: { kind: 'worksheet', key: 'opaque destination' },
    fromRevision: 4,
    toRevision: 5,
    causedBy: operationId,
    payload: {
      kind: 'resource_move',
      patch: { from: 'opaque source', to: 'opaque destination' },
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
    ['worksheet document', worksheetDelta],
    ['worksheet lifecycle', worksheetLifecycleDelta],
    ['worksheet move', worksheetMoveDelta],
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

  it('rejects nested extension fields from the strict graph DTO shape', () => {
    const delta = structuredClone(graphDelta()) as unknown as Record<string, any>;
    delta.resource.extension = true;
    delta.payload.extension = true;
    delta.payload.patch.extension = true;

    expect(areResourceDeltasValid([delta])).toBe(false);
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

describe('worksheet resource mutation wire', () => {
  it('accepts canonical document, lifecycle, and move deltas with opaque non-empty paths', () => {
    expect(areResourceDeltasValid([
      worksheetDelta(),
    ])).toBe(true);
    expect(areResourceDeltasValid([
      worksheetLifecycleDelta(),
    ])).toBe(true);
    expect(areResourceDeltasValid([
      worksheetMoveDelta(),
    ])).toBe(true);
  });

  it.each([
    ['empty resource path', (delta: Record<string, any>) => { delta.resource.key = ''; }],
    ['legacy document id', (delta: Record<string, any>) => { delta.payload.patch.after.id = 'legacy-id'; }],
    ['legacy document name', (delta: Record<string, any>) => {
      delta.payload.patch.after.name = 'persisted document name';
    }],
  ])('rejects %s', (_case, mutate) => {
    const delta = structuredClone(worksheetDelta()) as unknown as Record<string, any>;
    mutate(delta);
    expect(areResourceDeltasValid([delta])).toBe(false);
  });

  it('rejects a lifecycle state without its Rust-derived name', () => {
    const delta = structuredClone(worksheetLifecycleDelta()) as unknown as Record<string, any>;
    delete delta.payload.patch.after.name;
    expect(areResourceDeltasValid([delta])).toBe(false);
  });

  it.each([
    ['fresh create 0→0', 0, 0, null, 0],
    ['unload with unchanged authority 5→5', 5, 5, null, 5],
    ['reinsert over tombstone 5→6', 5, 6, null, 6],
    ['reinsert with an authoritative jump 5→9', 5, 9, null, 9],
    ['remove 5→6', 5, 6, 5, null],
  ])('accepts payload-aware lifecycle revisions for %s', (
    _case,
    fromRevision,
    toRevision,
    beforeRevision,
    afterRevision,
  ) => {
    const delta = worksheetLifecycleDelta() as unknown as Record<string, any>;
    delta.fromRevision = fromRevision;
    delta.toRevision = toRevision;
    delta.payload.patch.before = beforeRevision === null ? null : {
      revision: beforeRevision,
      path: delta.resource.key,
      kind: 'worksheet',
      name: 'Sales Report',
    };
    delta.payload.patch.after = afterRevision === null ? null : {
      revision: afterRevision,
      path: delta.resource.key,
      kind: 'worksheet',
      name: 'Sales Report',
    };

    expect(areResourceDeltasValid([delta])).toBe(true);
  });

  it.each([
    ['present after revision differs from envelope target', 5, 6, null, 5],
    ['present before revision differs from envelope source', 5, 6, 4, null],
    ['removal skips its tombstone successor', 5, 7, 5, null],
    ['reinsertion moves authority backward', 5, 4, null, 4],
  ])('rejects lifecycle revisions when %s', (
    _case,
    fromRevision,
    toRevision,
    beforeRevision,
    afterRevision,
  ) => {
    const delta = worksheetLifecycleDelta() as unknown as Record<string, any>;
    delta.fromRevision = fromRevision;
    delta.toRevision = toRevision;
    delta.payload.patch.before = beforeRevision === null ? null : {
      revision: beforeRevision,
      path: delta.resource.key,
      kind: 'worksheet',
      name: 'Sales Report',
    };
    delta.payload.patch.after = afterRevision === null ? null : {
      revision: afterRevision,
      path: delta.resource.key,
      kind: 'worksheet',
      name: 'Sales Report',
    };

    expect(areResourceDeltasValid([delta])).toBe(false);
  });

  it('keeps non-lifecycle document revisions strictly contiguous', () => {
    const delta = worksheetDelta();
    delta.toRevision = 6;

    expect(areResourceDeltasValid([delta])).toBe(false);
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
