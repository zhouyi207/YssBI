import { beforeEach, describe, expect, it } from 'vitest';
import { useGraphDataStore } from './graphDataStore';

describe('graphDataStore connection truth', () => {
  beforeEach(() => {
    useGraphDataStore.setState({
      nodes: {},
      pins: {},
      connections: {},
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
});
