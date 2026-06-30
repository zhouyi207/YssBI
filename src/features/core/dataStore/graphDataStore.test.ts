import { beforeEach, describe, expect, it } from 'vitest';
import { useGraphDataStore } from './graphDataStore';

describe('graphDataStore connection truth', () => {
  beforeEach(() => {
    useGraphDataStore.setState({
      nodes: {},
      pins: {},
      connections: {},
      graphEntities: {},
      graphNodes: {},
      nodePins: {},
      pinConnections: {},
    });
  });

  it('hydrates pinConnections from connections and ignores incoming pin links', () => {
    useGraphDataStore.getState().addGraphFromData('graph-1', {
      id: 'graph-1',
      name: 'Test',
      type: 'event',
      canvas: { x: 0, y: 0, scale: 1 },
      nodes: [
        {
          id: 'node-a',
          nodeType: 'Data:Constant',
          category: ['Data'],
          title: 'A',
          position: { x: 0, y: 0 },
          inputs: ['pin-in'],
          outputs: ['pin-out'],
        },
      ],
      pins: [
        {
          id: 'pin-in',
          nodeId: 'node-a',
          name: 'In',
          type: 'Float64',
          direction: 'input',
          links: ['should-be-ignored'],
        } as never,
        {
          id: 'pin-out',
          nodeId: 'node-a',
          name: 'Out',
          type: 'Float64',
          direction: 'output',
          links: ['should-be-ignored'],
        } as never,
      ],
      connections: {
        connections: [{ fromPin: 'pin-out', toPin: 'pin-in' }],
      },
    });

    const state = useGraphDataStore.getState();
    expect(state.pins['pin-in']).toBeDefined();
    expect(state.pins['pin-in']).not.toHaveProperty('links');
    expect(state.pinConnections['pin-out']).toEqual(['pin-out->pin-in']);
    expect(state.pinConnections['pin-in']).toEqual(['pin-out->pin-in']);
    expect(state.connections['pin-out->pin-in']).toEqual({
      id: 'pin-out->pin-in',
      from: 'pin-out',
      to: 'pin-in',
    });
  });

  it('clears a graph even when its graphNodes index contains stale node ids', () => {
    useGraphDataStore.setState({
      graphNodes: { 'graph-1': ['missing-node'] },
    });

    useGraphDataStore.getState().clearGraph('graph-1');

    expect(useGraphDataStore.getState().graphNodes['graph-1']).toBeUndefined();
  });

  it('keeps remaining graph bucket when graph-local node and pin ids overlap', () => {
    const graph = (id: string, title: string) => ({
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
          direction: 'input',
        },
        {
          id: 'local-out',
          nodeId: 'local-node',
          name: 'Out',
          type: 'Float64',
          direction: 'output',
        },
      ],
      connections: { connections: [{ fromPin: 'local-out', toPin: 'local-in' }] },
    });

    useGraphDataStore.getState().hydrateGraphs({
      'graph-1': graph('graph-1', 'First'),
      'graph-2': graph('graph-2', 'Second'),
    });

    useGraphDataStore.getState().clearGraph('graph-1');

    const state = useGraphDataStore.getState();
    expect(state.graphNodes['graph-1']).toBeUndefined();
    expect(state.graphNodes['graph-2']).toEqual(['local-node']);
    expect(state.getGraphNode('graph-2', 'local-node')?.title).toBe('Second');
    expect(state.getGraphPinConnections('graph-2', 'local-out')).toEqual(['local-out->local-in']);
  });
});
