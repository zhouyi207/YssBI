import { beforeEach, describe, expect, it } from 'vitest';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { makeOverlappingLocalIdGraphPair } from '@/tests/helpers/graphFixtures';
import { NodeDeletedHandler, PinTypesInferredHandler } from './NodeEventHandler';

describe('Node event handlers', () => {
  beforeEach(() => {
    useGraphDataStore.setState({ graphEntities: {} });
  });

  it('scopes node deletion by graph id when local node ids overlap', () => {
    useGraphDataStore.getState().hydrateGraphs(
      makeOverlappingLocalIdGraphPair(
        { id: 'graph-1', title: 'First' },
        { id: 'graph-2', title: 'Second' },
      ),
    );

    new NodeDeletedHandler().handle({ graphId: 'graph-1', nodeId: 'local-node' });

    const store = useGraphDataStore.getState();
    expect(store.getGraphNode('graph-1', 'local-node')).toBeUndefined();
    expect(store.getGraphNode('graph-2', 'local-node')?.title).toBe('Second');
  });

  it('scopes pin type updates by graph id when local pin ids overlap', () => {
    useGraphDataStore.getState().hydrateGraphs(
      makeOverlappingLocalIdGraphPair(
        { id: 'graph-1', title: 'First' },
        { id: 'graph-2', title: 'Second' },
      ),
    );

    new PinTypesInferredHandler().handle({
      graphId: 'graph-1',
      pinTypes: [
        {
          pinId: 'local-out',
          pinType: 'Int64',
        },
      ],
    });

    const store = useGraphDataStore.getState();
    expect(store.getGraphPin('graph-1', 'local-out')?.type).toBe('Int64');
    expect(store.getGraphPin('graph-2', 'local-out')?.type).toBe('Float64');
  });
});
