import { beforeEach, describe, expect, it } from 'vitest';
import { useGraphDataStore } from './graphDataStore';
import { makeOverlappingLocalIdGraphPair, makeTestGraph } from '@/tests/helpers/graphFixtures';

describe('graphDataStore connection truth', () => {
  beforeEach(() => {
    useGraphDataStore.setState({ graphEntities: {} });
  });

  it('hydrates pinConnections from connections and ignores incoming pin links', () => {
    const graph = makeTestGraph({
      path: 'graph-1',
      name: 'Test',
      title: 'A',
      nodeId: 'node-a',
      inputPinId: 'pin-in',
      outputPinId: 'pin-out',
    });
    (graph.pins[0] as { links?: string[] }).links = ['should-be-ignored'];

    useGraphDataStore.getState().addGraphFromData('graph-1', graph);

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

  it('isolates overlapping local ids across graph buckets', () => {
    const pair = makeOverlappingLocalIdGraphPair(
      { path: 'graph-1', title: 'First' },
      { path: 'graph-2', title: 'Second' },
    );
    useGraphDataStore.getState().addGraphFromData('graph-1', pair['graph-1']);
    useGraphDataStore.getState().addGraphFromData('graph-2', pair['graph-2']);

    const state = useGraphDataStore.getState();
    expect(state.getGraphNode('graph-1', 'local-node')?.title).toBe('First');
    expect(state.getGraphNode('graph-2', 'local-node')?.title).toBe('Second');
  });
});
