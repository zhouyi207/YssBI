import { beforeEach, describe, expect, it, vi } from 'vitest';
import { activateGraphTab, deactivateGraphTab } from './activateGraphTab';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import { useGraphDataStore, useProjectIOStore } from '@/features/core/dataStore';
import { markResourceLoaded } from '@/features/core/resource';
import { makeTestGraph } from '@/tests/helpers/graphFixtures';

describe('activateGraphTab', () => {
  const graphPath = 'events/Main.yssbi-event';

  beforeEach(() => {
    useGraphSessionStore.getState().reset();
    useGraphDataStore.setState({ graphEntities: {} });
    vi.restoreAllMocks();
  });

  it('calls loadGraph once and completes editor activation when graph is available', async () => {
    useGraphDataStore.getState().addGraphFromData(graphPath, makeTestGraph({ path: graphPath }));
    markResourceLoaded({ id: graphPath, kind: 'event' });

    const loadGraph = vi.fn(async () => true);
    useProjectIOStore.setState({ loadGraph });

    const ok = await activateGraphTab(graphPath, 'editor-1');

    expect(ok).toBe(true);
    expect(loadGraph).toHaveBeenCalledTimes(1);
    expect(loadGraph).toHaveBeenCalledWith(graphPath);
    expect(useGraphSessionStore.getState().getFocusedGraphPath()).toBe(graphPath);
  });

  it('rolls back session when loadGraph fails', async () => {
    const loadGraph = vi.fn(async () => false);
    useProjectIOStore.setState({ loadGraph });

    const ok = await activateGraphTab(graphPath, 'editor-1');

    expect(ok).toBe(false);
    expect(loadGraph).toHaveBeenCalledTimes(1);
    expect(useGraphSessionStore.getState().getFocusedGraphPath()).toBeNull();
  });
});

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
