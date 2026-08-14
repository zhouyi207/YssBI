import { beforeEach, describe, expect, it } from 'vitest';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { getCanvasInteraction, useGraphInteractionStore } from './graphInteractionStore';

describe('graphInteractionStore position overrides', () => {
  beforeEach(() => {
    useGraphInteractionStore.setState({ positionOverrides: {}, interactions: {} });
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

  it('keeps one interaction owner per graph and replaces the initiating pane', () => {
    const store = useGraphInteractionStore.getState();
    store.startInteraction('events/one', {
      type: 'selecting',
      session: { groupId: 'group-1', startX: 0, startY: 0, currentX: 0, currentY: 0, preserveSelection: false },
    });
    store.startInteraction('events/one', {
      type: 'panning',
      session: { groupId: 'group-2', startX: 0, startY: 0, lastX: 0, lastY: 0, moved: false },
    });
    store.startInteraction('events/two', {
      type: 'pendingNodeCreation',
      session: { groupId: 'group-3', graphPath: 'events/two', source: null, screenX: 10, screenY: 20 },
    });

    const state = useGraphInteractionStore.getState();
    expect(Object.keys(state.interactions).sort()).toEqual(['events/one', 'events/two']);
    expect(getCanvasInteraction(state, 'events/one', 'group-1')).toEqual({ type: 'idle' });
    expect(getCanvasInteraction(state, 'events/one', 'group-2').type).toBe('panning');
    expect(getCanvasInteraction(state, 'events/two', 'group-3').type).toBe('pendingNodeCreation');
  });

  it('cancels one graph interaction without changing installed projection', () => {
    const projectionBefore = useGraphDataStore.getState().graphEntities;
    const store = useGraphInteractionStore.getState();
    store.setPositionOverride('events/one', 'node-a', { x: 10, y: 20 });
    store.setPositionOverride('events/two', 'node-b', { x: 30, y: 40 });
    store.startInteraction('events/one', {
      type: 'draggingNodes',
      session: { groupId: 'group-1', nodeId: 'node-a', lastX: 0, lastY: 0, moved: false, nodeIds: ['node-a'], delta: { x: 0, y: 0 } },
    });

    expect(store.cancelInteraction('events/one', 'group-2')).toBe('idle');
    expect(getCanvasInteraction(useGraphInteractionStore.getState(), 'events/one', 'group-1').type).toBe('draggingNodes');
    expect(store.cancelInteraction('events/one', 'group-1')).toBe('draggingNodes');
    expect(getCanvasInteraction(useGraphInteractionStore.getState(), 'events/one', 'group-1')).toEqual({ type: 'idle' });
    expect(useGraphInteractionStore.getState().positionOverrides).toEqual({
      'events/two': { 'node-b': { x: 30, y: 40 } },
    });
    expect(useGraphDataStore.getState().graphEntities).toBe(projectionBefore);
  });
});
