import { describe, expect, it } from 'vitest';
import { makeOverlappingLocalIdGraphPair, makeTestGraph } from './graphFixtures';

describe('makeTestGraph', () => {
  it('builds connected local-id graph with typed pin directions', () => {
    const graph = makeTestGraph({ path: 'g1', title: 'One' });
    expect(graph.pins[0].direction).toBe('input');
    expect(graph.pins[1].direction).toBe('output');
    expect(graph.connections).toEqual([
      { id: 'local-out->local-in', from: 'local-out', to: 'local-in' },
    ]);
  });

  it('supports legacy pin links and custom pin ids', () => {
    const graph = makeTestGraph({
      path: 'g1',
      name: 'Test',
      title: 'A',
      nodeId: 'node-a',
      inputPinId: 'pin-in',
      outputPinId: 'pin-out',
      withLegacyPinLinks: true,
    });
    expect(graph.nodes[0].id).toBe('node-a');
    expect(graph.pins[0]).toMatchObject({ id: 'pin-in', links: ['should-be-ignored'] });
  });

  it('builds overlapping graph pairs for scope tests', () => {
    const pair = makeOverlappingLocalIdGraphPair(
      { path: 'graph-1', title: 'First' },
      { path: 'graph-2', title: 'Second' },
    );
    expect(Object.keys(pair)).toEqual(['graph-1', 'graph-2']);
    expect(pair['graph-1'].nodes[0].title).toBe('First');
  });
});
