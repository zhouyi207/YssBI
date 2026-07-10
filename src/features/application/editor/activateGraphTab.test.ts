import { beforeEach, describe, expect, it } from 'vitest';
import { deactivateGraphTab } from './activateGraphTab';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';

describe('deactivateGraphTab', () => {
  beforeEach(() => {
    useGraphSessionStore.getState().reset();
  });

  it('clears session when the closed tab owned the focused graph', () => {
    useGraphSessionStore.getState().setFocusedSession('editor', 'g1');

    deactivateGraphTab('editor', 'g1');

    expect(useGraphSessionStore.getState().getFocusedGraphPath()).toBeNull();
  });

  it('keeps session when a background tab is closed', () => {
    useGraphSessionStore.getState().setFocusedSession('editor', 'g1');

    deactivateGraphTab('editor', 'g2');

    expect(useGraphSessionStore.getState().getFocusedGraphPath()).toBe('g1');
  });
});
