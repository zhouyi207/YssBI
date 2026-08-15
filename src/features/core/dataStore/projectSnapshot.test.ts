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
    });
    expect(snapshot['graph-1'].nodes).toHaveLength(1);
    expect(snapshot['graph-1'].pins).toHaveLength(2);
    expect(snapshot['graph-1'].connections).toEqual([
      { from: 'pin-out', to: 'pin-in' },
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

  it('includes function signatures when getFunctionSignature is provided', () => {
    const snapshot = buildGraphSnapshot(
      makeAccess({
        getResourceMeta: () => ({ name: 'Compute', kind: 'function', exists: true }),
        getFunctionSignature: () => ({
          functionInputs: [{ id: 'in-1', name: 'A' }],
          functionOutputs: [{ id: 'out-1', name: 'R' }],
        }),
      }),
    );

    expect(snapshot['graph-1'].functionInputs).toEqual([{ id: 'in-1', name: 'A' }]);
    expect(snapshot['graph-1'].functionOutputs).toEqual([{ id: 'out-1', name: 'R' }]);
  });
});
