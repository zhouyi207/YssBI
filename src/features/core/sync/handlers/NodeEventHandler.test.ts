import { beforeEach, describe, expect, it } from 'vitest';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import type { GraphDataLike } from '@/shared/types/store/graph';
import { NodeDeletedHandler, PinTypesInferredHandler } from './NodeEventHandler';

const makeGraph = (id: string, title: string): GraphDataLike => ({
  id,
  name: title,
  type: 'event' as const,
  canvas: { x: 0, y: 0, scale: 1 },
  nodes: [
    {
      id: 'local-node',
      nodeType: 'Data:Constant',
      category: ['Data'],
      title,
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
      direction: 'input' as const,
    },
    {
      id: 'local-out',
      nodeId: 'local-node',
      name: 'Out',
      type: 'Float64',
      direction: 'output' as const,
    },
  ],
  connections: { connections: [{ fromPin: 'local-out', toPin: 'local-in' }] },
});

describe('Node event handlers', () => {
  beforeEach(() => {
    useGraphDataStore.setState({ graphEntities: {} });
  });

  it('scopes node deletion by graph id when local node ids overlap', () => {
    useGraphDataStore.getState().hydrateGraphs({
      'graph-1': makeGraph('graph-1', 'First'),
      'graph-2': makeGraph('graph-2', 'Second'),
    });

    new NodeDeletedHandler().handle({ graphId: 'graph-1', nodeId: 'local-node' });

    const store = useGraphDataStore.getState();
    expect(store.getGraphNode('graph-1', 'local-node')).toBeUndefined();
    expect(store.getGraphNode('graph-2', 'local-node')?.title).toBe('Second');
  });

  it('scopes pin type updates by graph id when local pin ids overlap', () => {
    useGraphDataStore.getState().hydrateGraphs({
      'graph-1': makeGraph('graph-1', 'First'),
      'graph-2': makeGraph('graph-2', 'Second'),
    });

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
