import { beforeEach, describe, expect, it } from 'vitest';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { makeTestGraph } from '@/tests/helpers/graphFixtures';
import { buildEdgeData } from './EdgesOverlay';

describe('EdgesOverlay', () => {
  beforeEach(() => {
    useGraphDataStore.setState({ graphEntities: {} });
  });

  it('renders edges from the active graph bucket when local pin ids overlap', () => {
    useGraphDataStore.getState().hydrateGraphs({
      'graph-1': makeTestGraph({ id: 'graph-1', outputPinColor: '#ff0000' }),
      'graph-2': makeTestGraph({ id: 'graph-2', outputPinColor: '#0000ff' }),
    });

    const store = useGraphDataStore.getState();
    const edges = buildEdgeData(
      store.getGraphNodeIds('graph-1'),
      store.getGraphConnections('graph-1'),
      (pinId) => store.getGraphPin('graph-1', pinId),
    );

    expect(edges).toEqual([
      expect.objectContaining({
        id: 'local-out->local-in',
        pinColor: '#ff0000',
      }),
    ]);
  });
});
