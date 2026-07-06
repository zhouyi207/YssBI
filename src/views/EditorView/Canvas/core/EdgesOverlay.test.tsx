import { beforeEach, describe, expect, it } from 'vitest';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { buildEdgeData } from './EdgesOverlay';

const makeGraph = (graphId: string, color: string) => ({
  id: graphId,
  name: graphId,
  type: 'event' as const,
  canvas: { x: 0, y: 0, scale: 1 },
  nodes: [
    {
      id: 'local-node',
      nodeType: 'Data:Constant',
      category: ['Data'],
      title: graphId,
      position: { x: 0, y: 0 },
      inputs: ['local-in'],
      outputs: ['local-out'],
    },
  ],
  pins: [
    {
      id: 'local-in',
      nodeId: 'local-node',
      name: 'In',
      type: 'Float64',
      direction: 'input',
    },
    {
      id: 'local-out',
      nodeId: 'local-node',
      name: 'Out',
      type: 'Float64',
      direction: 'output',
      ui: { color },
    },
  ],
  connections: { connections: [{ fromPin: 'local-out', toPin: 'local-in' }] },
});

describe('EdgesOverlay', () => {
  beforeEach(() => {
    useGraphDataStore.setState({ graphEntities: {} });
  });

  it('renders edges from the active graph bucket when local pin ids overlap', () => {
    useGraphDataStore.getState().hydrateGraphs({
      'graph-1': makeGraph('graph-1', '#ff0000'),
      'graph-2': makeGraph('graph-2', '#0000ff'),
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
