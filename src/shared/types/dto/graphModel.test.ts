import { describe, expect, it } from 'vitest';
import { makeTestGraph } from '@/tests/helpers/graphFixtures';
import {
  domainGraphToGraphData,
  graphDataToDomainGraph,
  graphInstanceDtoToGraphData,
  normalizeGraphConnections,
  normalizeGraphDataLike,
} from './graphModel';
import type { GraphData, PinData } from '../store/graph';

function makeGraphData(): GraphData {
  return normalizeGraphDataLike(
    'graph-1',
    makeTestGraph({
      path: 'graph-1',
      name: 'Main',
      title: 'A',
      nodeId: 'node-a',
      inputPinId: 'pin-in',
      outputPinId: 'pin-out',
    }),
  );
}

describe('graphModel converters', () => {
  it('graphDataToDomainGraph embeds pins on nodes and wraps connections', () => {
    const domain = graphDataToDomainGraph(makeGraphData());

    expect(domain.nodes[0].inputs).toHaveLength(1);
    expect(domain.nodes[0].inputs[0].id).toBe('pin-in');
    expect(domain.nodes[0].outputs[0].id).toBe('pin-out');
    expect(domain.pins).toHaveLength(2);
    expect(domain.connections).toEqual({
      connections: [{ fromPin: 'pin-out', toPin: 'pin-in' }],
    });
  });

  it('domainGraphToGraphData round-trips store node metadata', () => {
    const domain = graphDataToDomainGraph(makeGraphData());
    const restored = domainGraphToGraphData(domain);

    expect(restored.nodes[0]).toMatchObject({
      id: 'node-a',
      graphPath: 'graph-1',
      position: { x: 0, y: 0 },
      inputs: ['pin-in'],
      outputs: ['pin-out'],
    });
    expect(restored.connections).toEqual([
      { id: 'pin-out->pin-in', from: 'pin-out', to: 'pin-in' },
    ]);
  });

  it('normalizeGraphConnections accepts wrapped and array hydrate inputs', () => {
    const wrapped = normalizeGraphConnections({
      connections: [{ fromPin: 'a', toPin: 'b' }],
    });
    expect(wrapped).toEqual([{ id: 'a->b', from: 'a', to: 'b' }]);

    const array = normalizeGraphConnections([{ id: 'c->d', from: 'c', to: 'd' }]);
    expect(array).toEqual([{ id: 'c->d', from: 'c', to: 'd' }]);
  });

  it('normalizeGraphDataLike accepts RuntimeNodeInput with embedded pin objects', () => {
    const base = makeTestGraph({
      path: 'g1',
      nodeId: 'n1',
      inputPinId: 'in-1',
      outputPinId: 'out-1',
      connected: false,
    });
    const pinIn = base.pins![0] as PinData;
    const pinOut = base.pins![1] as PinData;

    const normalized = normalizeGraphDataLike('g1', {
      path: 'g1',
      name: 'g1',
      type: 'event',
      pins: [...(base.pins ?? [])] as PinData[],
      connections: base.connections as GraphData['connections'],
      nodes: [
        {
          id: 'n1',
          nodeType: 'Data:Constant',
          inputs: [pinIn],
          outputs: [pinOut],
        },
      ],
    });

    expect(normalized.nodes[0].inputs).toEqual(['in-1']);
    expect(normalized.nodes[0].outputs).toEqual(['out-1']);
  });

  it('graphInstanceDtoToGraphData + graphDataToDomainGraph matches hydrate shape', () => {
    const dtoGraph = graphDataToDomainGraph(
      graphInstanceDtoToGraphData({
        path: 'evt-1',
        name: 'Event',
        type: 'event',
        nodes: [
          {
            id: 'node-a',
            nodeType: 'Control:Begin',
            category: ['Control'],
            title: 'Begin',
            inputs: [],
            outputs: ['pin-exec'],
            position: { x: 0, y: 0 },
            paramsKind: 'none',
          },
        ],
        pins: [
          {
            id: 'pin-exec',
            nodeId: 'node-a',
            name: 'Exec',
            type: 'exec',
            direction: 'output',
          },
        ],
        connections: { connections: [] },
      }),
    );

    expect(dtoGraph.nodes[0].outputs[0].type).toBe('exec');
    expect(dtoGraph.connections.connections).toEqual([]);
  });
});
