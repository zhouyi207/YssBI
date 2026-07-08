import { describe, expect, it } from 'vitest';
import { buildGraphSnapshot } from './projectSnapshot';
import type { GraphData } from '@/shared/types/store/graph';

function makeAccess(overrides: Partial<Parameters<typeof buildGraphSnapshot>[0]> = {}) {
  const node: GraphData['nodes'][number] = {
    id: 'node-1',
    graphId: 'graph-1',
    nodeType: 'Data:Constant',
    category: ['Data'],
    title: 'Const',
    inputs: ['pin-in'],
    outputs: ['pin-out'],
    uiStyle: 'default',
    position: { x: 0, y: 0 },
  };
  const pins: GraphData['pins'] = [
    {
      id: 'pin-in',
      nodeId: 'node-1',
      name: 'In',
      type: 'Float64',
      direction: 'input',
    },
    {
      id: 'pin-out',
      nodeId: 'node-1',
      name: 'Out',
      type: 'Float64',
      direction: 'output',
    },
  ];

  return {
    graphOrder: ['graph-1', 'missing-graph'],
    getResourceMeta: (graphId: string) =>
      graphId === 'missing-graph'
        ? null
        : { name: 'Main Event', kind: 'event' as const, exists: true },
    getGraphNodeIds: () => ['node-1'],
    getGraphNode: () => node,
    getGraphNodePins: () => ['pin-in', 'pin-out'],
    getGraphPin: (_graphId: string, pinId: string) =>
      pins.find((pin) => pin.id === pinId) ?? null,
    getGraphPinConnections: (_graphId: string, pinId: string) =>
      pinId === 'pin-out' ? ['pin-out->pin-in'] : ['pin-out->pin-in'],
    getGraphConnection: () => ({ from: 'pin-out', to: 'pin-in' }),
    getViewport: () => ({ x: 12, y: 34, scale: 1.5 }),
    ...overrides,
  };
}

describe('buildGraphSnapshot', () => {
  it('exports nodes, pins, and wrapped connections for known graphs', () => {
    const snapshot = buildGraphSnapshot(makeAccess());

    expect(Object.keys(snapshot)).toEqual(['graph-1']);
    expect(snapshot['graph-1']).toMatchObject({
      id: 'graph-1',
      name: 'Main Event',
      type: 'event',
      canvas: { x: 12, y: 34, scale: 1.5 },
    });
    expect(snapshot['graph-1'].nodes).toHaveLength(1);
    expect(snapshot['graph-1'].pins).toHaveLength(2);
    expect(snapshot['graph-1'].connections).toEqual([
      { id: 'pin-out->pin-in', from: 'pin-out', to: 'pin-in' },
    ]);
  });

  it('respects graphOrder and skips graphs without resource meta', () => {
    const snapshot = buildGraphSnapshot(
      makeAccess({
        graphOrder: ['graph-b', 'graph-a'],
        getResourceMeta: (graphId) => ({
          name: graphId,
          kind: 'function',
          exists: graphId !== 'graph-b',
        }),
      }),
    );

    expect(Object.keys(snapshot)).toEqual(['graph-a']);
    expect(snapshot['graph-a'].type).toBe('function');
  });
});
