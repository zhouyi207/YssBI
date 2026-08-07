import { describe, expect, it } from 'vitest';
import { makeTestGraph } from '@/tests/helpers/graphFixtures';
import {
  graphDataRecordToDomainGraphs,
  graphDataToDomainGraph,
} from './graphModel';

describe('graphModel converters', () => {
  it('graphDataToDomainGraph embeds pins on nodes and wraps connections', () => {
    const graph = makeTestGraph({
      path: 'graph-1',
      name: 'Main',
      title: 'A',
      nodeId: 'node-a',
      inputPinId: 'pin-in',
      outputPinId: 'pin-out',
    });

    const domain = graphDataToDomainGraph(graph);

    expect(domain.nodes[0].inputs).toHaveLength(1);
    expect(domain.nodes[0].inputs[0].id).toBe('pin-in');
    expect(domain.nodes[0].outputs[0].id).toBe('pin-out');
    expect(domain.pins).toHaveLength(2);
    expect(domain.connections).toEqual({
      connections: [{ fromPin: 'pin-out', toPin: 'pin-in' }],
    });
  });

  it('converts only canonical GraphData records to domain graphs', () => {
    const graph = makeTestGraph({ path: 'events/main', name: 'Main' });

    const converted = graphDataRecordToDomainGraphs({ [graph.path]: graph });

    expect(Object.keys(converted)).toEqual(['events/main']);
    expect(converted['events/main']).toMatchObject({
      path: 'events/main',
      name: 'Main',
      type: 'event',
    });
  });
});
