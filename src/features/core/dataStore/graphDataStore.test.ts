import { beforeEach, describe, expect, it } from 'vitest';
import { useGraphDataStore } from './graphDataStore';
import { makeOverlappingLocalIdGraphPair, makeTestGraph } from '@/tests/helpers/graphFixtures';

describe('graphDataStore connection truth', () => {
  beforeEach(() => {
    useGraphDataStore.setState({ graphEntities: {} });
  });

  it('hydrates pinConnections from connections and ignores incoming pin links', () => {
    useGraphDataStore.getState().addGraphFromData(
      'graph-1',
      makeTestGraph({
        path: 'graph-1',
        name: 'Test',
        title: 'A',
        nodeId: 'node-a',
        inputPinId: 'pin-in',
        outputPinId: 'pin-out',
        withLegacyPinLinks: true,
      }),
    );

    const state = useGraphDataStore.getState();
    const bucket = state.graphEntities['graph-1'];
    expect(bucket.pins['pin-in']).toBeDefined();
    expect(bucket.pins['pin-in']).not.toHaveProperty('links');
    expect(bucket.pinConnections['pin-out']).toEqual(['pin-out->pin-in']);
    expect(bucket.pinConnections['pin-in']).toEqual(['pin-out->pin-in']);
    expect(bucket.connections['pin-out->pin-in']).toEqual({
      id: 'pin-out->pin-in',
      from: 'pin-out',
      to: 'pin-in',
    });
  });

  it('clearGraph on missing graph is a no-op', () => {
    useGraphDataStore.getState().clearGraph('graph-1');
    expect(useGraphDataStore.getState().hasGraph('graph-1')).toBe(false);
  });

  it('keeps remaining graph bucket when graph-local node and pin ids overlap', () => {
    useGraphDataStore.getState().hydrateGraphs(
      makeOverlappingLocalIdGraphPair(
        { path: 'graph-1', title: 'First' },
        { path: 'graph-2', title: 'Second' },
      ),
    );

    useGraphDataStore.getState().clearGraph('graph-1');

    const state = useGraphDataStore.getState();
    expect(state.hasGraph('graph-1')).toBe(false);
    expect(state.getGraphNodeIds('graph-2')).toEqual(['local-node']);
    expect(state.getGraphNode('graph-2', 'local-node')?.title).toBe('Second');
    expect(state.getGraphPinConnections('graph-2', 'local-out')).toEqual(['local-out->local-in']);
  });
});
