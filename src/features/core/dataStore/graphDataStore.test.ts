import { beforeEach, describe, expect, it } from 'vitest';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import {
  commitPreparedGraphProjectionReplacements,
  prepareGraphProjectionReplacements,
  useGraphDataStore,
} from './graphDataStore';

describe('graphDataStore projected entity truth', () => {
  beforeEach(() => {
    useGraphDataStore.setState({ graphEntities: {} });
  });

  it('stores projected connection topology and required metadata', () => {
    const fixture = makeEditorProjectionFixture({ graphPath: 'graph-1' });

    useGraphDataStore.getState().replaceProjection('graph-1', fixture.projection, 7);

    const bucket = useGraphDataStore.getState().graphEntities['graph-1'];
    expect(bucket.pinConnections[fixture.outputKey]).toEqual(['local-connection']);
    expect(bucket.pinConnections[fixture.inputKey]).toEqual(['local-connection']);
    expect(bucket.connections['local-connection']).toMatchObject({
      id: 'local-connection',
      from: fixture.outputKey,
      to: fixture.inputKey,
    });
    expect(bucket).toMatchObject({
      basis: fixture.projection.basis,
      sourceRevision: 1,
      requestGeneration: 7,
      diagnostics: [],
      hasBlockingDiagnostics: false,
    });
  });

  it('atomically accepts an authoritative dependency replacement at the same graph revision', () => {
    const current = makeEditorProjectionFixture({
      graphPath: 'functions/Compute.yssbi-function',
      sourceRevision: 7,
      title: 'Before signature',
    });
    const committed = makeEditorProjectionFixture({
      graphPath: 'functions/Compute.yssbi-function',
      sourceRevision: 7,
      title: 'After signature',
    });
    useGraphDataStore.getState().replaceProjection(
      'functions/Compute.yssbi-function',
      current.projection,
      1,
    );

    const outcome = useGraphDataStore.getState().replaceProjectionsAtomically([{
      graphPath: 'functions/Compute.yssbi-function',
      projection: committed.projection,
    }]);

    expect(outcome).toEqual({
      applied: true,
      graphPaths: ['functions/Compute.yssbi-function'],
    });
    expect(useGraphDataStore.getState().graphEntities['functions/Compute.yssbi-function'])
      .toMatchObject({
        sourceRevision: 7,
        nodes: { 'local-node': { title: 'After signature' } },
      });
  });

  it('prepares malformed projection replacements with zero store effects', () => {
    const current = makeEditorProjectionFixture({ graphPath: 'graph-1', title: 'Current' });
    useGraphDataStore.getState().replaceProjection('graph-1', current.projection, 1);
    const before = useGraphDataStore.getState().graphEntities;
    const malformed = structuredClone(current.projection);
    malformed.nodes[0].graphPath = 'graph-other';

    const prepared = prepareGraphProjectionReplacements([{
      graphPath: 'graph-1',
      projection: malformed,
    }]);

    expect(prepared).toMatchObject({ prepared: false, graphPath: 'graph-1' });
    expect(useGraphDataStore.getState().graphEntities).toBe(before);
  });

  it('commits a prepared replacement through one non-failing Zustand write', () => {
    const current = makeEditorProjectionFixture({ graphPath: 'graph-1', title: 'Current' });
    const replacement = makeEditorProjectionFixture({ graphPath: 'graph-1', title: 'Prepared' });
    useGraphDataStore.getState().replaceProjection('graph-1', current.projection, 1);
    const prepared = prepareGraphProjectionReplacements([{
      graphPath: 'graph-1',
      projection: replacement.projection,
    }]);
    if (!prepared.prepared) throw new Error('expected projection preparation to succeed');
    expect(useGraphDataStore.getState().graphEntities['graph-1'].nodes['local-node'].title)
      .toBe('Current');

    expect(() => commitPreparedGraphProjectionReplacements(prepared.plan)).not.toThrow();

    expect(useGraphDataStore.getState().graphEntities['graph-1'].nodes['local-node'].title)
      .toBe('Prepared');
  });

  it('isolates overlapping local ids across projected graph buckets', () => {
    const first = makeEditorProjectionFixture({ graphPath: 'graph-1', title: 'First' });
    const second = makeEditorProjectionFixture({ graphPath: 'graph-2', title: 'Second' });
    const store = useGraphDataStore.getState();
    store.replaceProjection('graph-1', first.projection, 1);
    store.replaceProjection('graph-2', second.projection, 1);

    expect(store.getGraphNode('graph-1', 'local-node')?.title).toBe('First');
    expect(store.getGraphNode('graph-2', 'local-node')?.title).toBe('Second');
  });

});
