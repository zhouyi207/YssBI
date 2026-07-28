import { beforeEach, describe, expect, it } from 'vitest';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import { buildEdgeData } from './EdgesOverlay';

describe('EdgesOverlay', () => {
  beforeEach(() => {
    useGraphDataStore.setState({ graphEntities: {} });
  });

  it('renders edges from the requested projected graph when local ids overlap', () => {
    const first = makeEditorProjectionFixture({ graphPath: 'graph-1' });
    const second = makeEditorProjectionFixture({ graphPath: 'graph-2' });
    const store = useGraphDataStore.getState();
    store.replaceProjection('graph-1', first.projection, 1);
    store.replaceProjection('graph-2', second.projection, 1);

    const edges = buildEdgeData(
      store.getGraphNodeIds('graph-1'),
      store.getGraphConnections('graph-1'),
      (pinId) => store.getGraphPin('graph-1', pinId),
    );

    expect(edges).toEqual([
      expect.objectContaining({
        id: 'local-connection',
        fromPinId: first.outputKey,
        toPinId: first.inputKey,
        sourceNodeId: 'local-node',
      }),
    ]);
  });
});
