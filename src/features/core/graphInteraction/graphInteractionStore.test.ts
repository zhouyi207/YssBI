import { beforeEach, describe, expect, it } from 'vitest';
import { useGraphInteractionStore } from './graphInteractionStore';

describe('graphInteractionStore position overrides', () => {
  beforeEach(() => {
    useGraphInteractionStore.setState({ positionOverrides: {} });
  });

  it('keeps overrides isolated by graph and clears only selected nodes', () => {
    const store = useGraphInteractionStore.getState();
    store.setPositionOverride('events/one', 'node-a', { x: 10, y: 20 });
    store.setPositionOverride('events/one', 'node-b', { x: 30, y: 40 });
    store.setPositionOverride('events/two', 'node-a', { x: 50, y: 60 });

    store.clearPositionOverrides('events/one', ['node-a']);

    expect(useGraphInteractionStore.getState().positionOverrides).toEqual({
      'events/one': { 'node-b': { x: 30, y: 40 } },
      'events/two': { 'node-a': { x: 50, y: 60 } },
    });
  });

  it('clears all temporary interaction state for one graph', () => {
    const store = useGraphInteractionStore.getState();
    store.setPositionOverride('events/one', 'node-a', { x: 10, y: 20 });
    store.setPositionOverride('events/two', 'node-b', { x: 30, y: 40 });

    store.clearGraphInteraction('events/one');

    expect(useGraphInteractionStore.getState().positionOverrides).toEqual({
      'events/two': { 'node-b': { x: 30, y: 40 } },
    });
  });
});
