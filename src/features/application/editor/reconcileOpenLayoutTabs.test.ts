import { beforeEach, describe, expect, it } from 'vitest';
import { useEditorTabStore } from '@/features/core/layout/editorTabStore';
import { resourceKey, useResourceStore } from '@/features/core/resource';
import { reconcileOpenLayoutTabsWithResources } from './reconcileOpenLayoutTabs';

describe('reconcileOpenLayoutTabsWithResources', () => {
  beforeEach(() => {
    useEditorTabStore.setState({
      registry: {},
      placements: {},
    });
    useResourceStore.getState().clear();
  });

  it.each(['node', 'connection'] as const)(
    'clears %s selection when removing the active graph selects another tab',
    (selectionKind) => {
      useResourceStore.getState().upsertResource({
        id: 'events/kept.yssbi-event',
        kind: 'event',
        name: 'Kept',
        uri: resourceKey({ id: 'events/kept.yssbi-event', kind: 'event' }),
        exists: true,
        loaded: false,
        hasDirtyDocument: false,
        hasStaleDocument: false,
        hasConflictDocument: false,
      });
      useEditorTabStore.getState().initGroupPlacement('editor', [
        { id: 'events/kept.yssbi-event', component: 'GraphEditor', type: 'event' },
        { id: 'events/missing.yssbi-event', component: 'GraphEditor', type: 'event' },
      ], 'events/missing.yssbi-event');
      if (selectionKind === 'node') {
        useEditorTabStore.getState().setSelectedNodeIds('editor', ['node-a']);
      } else {
        useEditorTabStore.getState().setSelectedConnectionIds('editor', ['edge-a']);
      }

      reconcileOpenLayoutTabsWithResources();

      expect(useEditorTabStore.getState().getPlacement('editor')).toMatchObject({
        activeTabId: 'events/kept.yssbi-event',
        selectedNodeIds: [],
        selectedConnectionIds: [],
      });
    },
  );

  it('removes persisted graph tabs absent from the current project index', () => {
    useEditorTabStore.getState().initGroupPlacement('editor', [
      { id: 'events/missing.yssbi-event', component: 'GraphEditor', type: 'event' },
    ]);

    reconcileOpenLayoutTabsWithResources();

    expect(useEditorTabStore.getState().getPlacement('editor').tabIds).toEqual([]);
    expect(useEditorTabStore.getState().resolveTab('events/missing.yssbi-event')).toBeNull();
  });
});
