import { beforeEach, describe, expect, it } from 'vitest';
import { useEditorTabStore } from '@/features/core/layout/editorTabStore';
import { useResourceStore } from '@/features/core/resource';
import { reconcileOpenLayoutTabsWithResources } from './reconcileOpenLayoutTabs';

describe('reconcileOpenLayoutTabsWithResources', () => {
  beforeEach(() => {
    useEditorTabStore.setState({
      registry: {},
      placements: {},
    });
    useResourceStore.getState().clear();
  });

  it('removes persisted graph tabs absent from the current project index', () => {
    useEditorTabStore.getState().initGroupPlacement('editor', [
      { id: 'events/missing.yssbi-event', component: 'GraphEditor', type: 'event' },
    ]);

    reconcileOpenLayoutTabsWithResources();

    expect(useEditorTabStore.getState().getPlacement('editor').tabIds).toEqual([]);
    expect(useEditorTabStore.getState().resolveTab('events/missing.yssbi-event')).toBeNull();
  });
});
