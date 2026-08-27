import { describe, expect, it } from 'vitest';
import { getNodeSelectionOptions, type NodeSelectionGraphSnapshot } from './nodeSelectionOptions';

const graph = {
  graphNodes: ['node-a', 'missing-node'],
  nodes: {
    'node-a': { id: 'node-a', title: 'First node' },
  },
} satisfies NodeSelectionGraphSnapshot;

describe('node selection options', () => {
  it('reuses the options snapshot for an unchanged graph projection', () => {
    const first = getNodeSelectionOptions(graph);
    const second = getNodeSelectionOptions(graph);

    expect(first).toEqual([{ id: 'node-a', title: 'First node' }]);
    expect(second).toBe(first);
  });

  it('reuses the empty snapshot when there is no graph projection', () => {
    expect(getNodeSelectionOptions()).toBe(getNodeSelectionOptions());
  });
});
