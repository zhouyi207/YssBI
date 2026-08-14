// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { getCanvasInteraction, useGraphInteractionStore } from '@/features/core/graphInteraction/graphInteractionStore';
import {
  cancelCanvasInteraction,
  clearCanvasInteractionGraph,
  clearCanvasInteractionProject,
  registerCanvasInteractionCleanup,
  resetCanvasInteractionCleanupForTests,
  startCanvasInteraction,
} from './canvasInteractionCleanup';

const graphPath = 'events/main';
const groupId = 'group-a';

afterEach(() => {
  resetCanvasInteractionCleanupForTests();
  useGraphInteractionStore.setState({ interactions: {}, positionOverrides: {} });
  document.body.innerHTML = '';
});

describe('canvasInteractionCleanup', () => {
  it('runs registered selection DOM cleanup before returning the interaction to idle', () => {
    document.body.innerHTML = `<div data-editor-group-id="${groupId}"><div data-selection-preview="true"></div></div>`;
    const canvas = document.querySelector(`[data-editor-group-id="${groupId}"]`)!;
    useGraphInteractionStore.getState().startInteraction(graphPath, {
      type: 'selecting',
      session: { groupId, startX: 0, startY: 0, currentX: 10, currentY: 10, preserveSelection: false },
    });
    const unregister = registerCanvasInteractionCleanup(
      { graphPath, groupId, interactionType: 'selecting' },
      () => canvas.querySelectorAll('[data-selection-preview]').forEach((element) => element.removeAttribute('data-selection-preview')),
    );

    expect(cancelCanvasInteraction(graphPath, groupId)).toBe('selecting');
    expect(canvas.querySelector('[data-selection-preview]')).toBeNull();
    expect(getCanvasInteraction(useGraphInteractionStore.getState(), graphPath, groupId)).toEqual({ type: 'idle' });
    unregister();
  });

  it('consumes a registered cleanup once so a repeated selection cannot call an old closure', () => {
    const cleanup = vi.fn();
    registerCanvasInteractionCleanup(
      { graphPath, groupId, interactionType: 'selecting' },
      cleanup,
    );
    useGraphInteractionStore.getState().startInteraction(graphPath, {
      type: 'selecting',
      session: { groupId, startX: 0, startY: 0, currentX: 1, currentY: 1, preserveSelection: false },
    });
    cancelCanvasInteraction(graphPath, groupId);

    useGraphInteractionStore.getState().startInteraction(graphPath, {
      type: 'selecting',
      session: { groupId, startX: 0, startY: 0, currentX: 2, currentY: 2, preserveSelection: false },
    });
    cancelCanvasInteraction(graphPath, groupId);

    expect(cleanup).toHaveBeenCalledOnce();
  });

  it('cancels the previous pane cleanup before another pane owns the same graph', () => {
    const cleanup = vi.fn();
    useGraphInteractionStore.getState().startInteraction(graphPath, {
      type: 'selecting',
      session: { groupId, startX: 0, startY: 0, currentX: 4, currentY: 4, preserveSelection: false },
    });
    registerCanvasInteractionCleanup(
      { graphPath, groupId, interactionType: 'selecting' },
      cleanup,
    );

    startCanvasInteraction(graphPath, {
      type: 'panning',
      session: { groupId: 'group-b', startX: 0, startY: 0, lastX: 0, lastY: 0, moved: false },
    });

    expect(cleanup).toHaveBeenCalledOnce();
    expect(getCanvasInteraction(useGraphInteractionStore.getState(), graphPath, groupId)).toEqual({ type: 'idle' });
    expect(getCanvasInteraction(useGraphInteractionStore.getState(), graphPath, 'group-b').type).toBe('panning');
  });

  it('does not run an unmounted cleanup for a later interaction in the same scope', () => {
    const cleanup = vi.fn();
    const unregister = registerCanvasInteractionCleanup(
      { graphPath, groupId, interactionType: 'selecting' },
      cleanup,
    );
    unregister();
    useGraphInteractionStore.getState().startInteraction(graphPath, {
      type: 'selecting',
      session: { groupId, startX: 0, startY: 0, currentX: 3, currentY: 3, preserveSelection: false },
    });

    cancelCanvasInteraction(graphPath, groupId);

    expect(cleanup).not.toHaveBeenCalled();
  });

  it('clears active graph cleanup, interaction, and position overrides through one lifecycle API', () => {
    const cleanup = vi.fn();
    useGraphInteractionStore.getState().startInteraction(graphPath, {
      type: 'draggingNodes',
      session: { groupId, nodeId: 'node-1', lastX: 0, lastY: 0, moved: true, nodeIds: ['node-1'], delta: { x: 1, y: 2 } },
    });
    useGraphInteractionStore.getState().setPositionOverride(graphPath, 'node-1', { x: 1, y: 2 });
    registerCanvasInteractionCleanup(
      { graphPath, groupId, interactionType: 'draggingNodes' },
      cleanup,
    );

    clearCanvasInteractionGraph(graphPath);

    expect(cleanup).toHaveBeenCalledOnce();
    expect(useGraphInteractionStore.getState().interactions[graphPath]).toBeUndefined();
    expect(useGraphInteractionStore.getState().positionOverrides[graphPath]).toBeUndefined();
  });

  it('clears every registered graph during project reset', () => {
    const first = vi.fn();
    const second = vi.fn();
    registerCanvasInteractionCleanup(
      { graphPath: 'events/one', groupId, interactionType: 'selecting' },
      first,
    );
    registerCanvasInteractionCleanup(
      { graphPath: 'events/two', groupId, interactionType: 'selecting' },
      second,
    );
    useGraphInteractionStore.setState({
      interactions: {
        'events/one': { type: 'selecting', session: { groupId, startX: 0, startY: 0, currentX: 1, currentY: 1, preserveSelection: false } },
        'events/two': { type: 'selecting', session: { groupId, startX: 0, startY: 0, currentX: 1, currentY: 1, preserveSelection: false } },
      },
      positionOverrides: {
        'events/one': { node: { x: 1, y: 1 } },
        'events/two': { node: { x: 2, y: 2 } },
      },
    });

    clearCanvasInteractionProject();

    expect(first).toHaveBeenCalledOnce();
    expect(second).toHaveBeenCalledOnce();
    expect(useGraphInteractionStore.getState()).toMatchObject({ interactions: {}, positionOverrides: {} });
  });

  it('does not run a cleanup registered for another pane', () => {
    let called = false;
    const unregister = registerCanvasInteractionCleanup(
      { graphPath, groupId: 'group-b', interactionType: 'selecting' },
      () => { called = true; },
    );
    useGraphInteractionStore.getState().startInteraction(graphPath, {
      type: 'selecting',
      session: { groupId, startX: 0, startY: 0, currentX: 0, currentY: 0, preserveSelection: false },
    });
    cancelCanvasInteraction(graphPath, groupId);
    expect(called).toBe(false);
    unregister();
  });
});
