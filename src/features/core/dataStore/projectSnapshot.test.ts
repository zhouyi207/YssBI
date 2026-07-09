import { describe, expect, it } from 'vitest';
import { buildGraphSnapshot } from './projectSnapshot';
import type { GraphData } from '@/shared/types/store/graph';

function makeAccess(overrides: Partial<Parameters<typeof buildGraphSnapshot>[0]> = {}) {
  const node: GraphData['nodes'][number] = {
    id: 'node-1',
    graphPath: 'graph-1',
    nodeType: 'Data:Constant',
    category: ['Data'],
    title: 'Const',
    inputs: ['pin-in'],
    outputs: ['pin-out'],
    position: { x: 0, y: 0 },
  };
  const pins: GraphData['pins'] = [
    {
      id: 'pin-in',
      nodeId: 'node-1',
      name: 'In',
      type: 'object',
      direction: 'input',
      dataType: { kind: 'Float64' },
    },
    {
      id: 'pin-out',
      nodeId: 'node-1',
      name: 'Out',
      type: 'object',
      direction: 'output',
      dataType: { kind: 'Float64' },
    },
  ];

  return {
    graphOrder: ['graph-1', 'missing-graph'],
    getResourceMeta: (graphPath: string) =>
      graphPath === 'missing-graph'
        ? null
        : { name: 'Main Event', kind: 'event' as const, exists: true },
    getGraphNodeIds: () => ['node-1'],
    getGraphNode: () => node,
    getGraphNodePins: () => ['pin-in', 'pin-out'],
    getGraphPin: (_graphPath: string, pinId: string) =>
      pins.find((pin) => pin.id === pinId) ?? null,
    getGraphPinConnections: (_graphPath: string, pinId: string) =>
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
      path: 'graph-1',
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
        getResourceMeta: (graphPath) => ({
          name: graphPath,
          kind: 'function',
          exists: graphPath !== 'graph-b',
        }),
      }),
    );

    expect(Object.keys(snapshot)).toEqual(['graph-a']);
    expect(snapshot['graph-a'].type).toBe('function');
  });
});
