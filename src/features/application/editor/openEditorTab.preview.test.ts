import { beforeEach, describe, expect, it } from 'vitest';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { useEditorTabStore } from '@/features/core/layout/editorTabStore';
import { buildGraphLayoutTab } from '@/features/core/layout/layoutTabModel';
import { openEditorTab } from './openEditorTab';

describe('openEditorTab preview', () => {
  beforeEach(() => {
    useEditorTabStore.setState({ registry: {}, placements: {} });
    useLayoutStore.setState({
      nodes: {
        default_editor: {
          id: 'default_editor',
          type: 'component',
          parentId: 'center',
          data: { component: 'GraphEditor' },
        },
      },
      activeEditorGroupId: 'default_editor',
    } as Partial<ReturnType<typeof useLayoutStore.getState>>);
    useEditorTabStore.getState().ensureGroupPlacement('default_editor');
  });

  it('replaces the preview tab when opening another preview in the same group', () => {
    openEditorTab(buildGraphLayoutTab('events/A.yssbi-event', 'event', { pinned: false }), {
      pinned: false,
    });
    openEditorTab(buildGraphLayoutTab('events/B.yssbi-event', 'event', { pinned: false }), {
      pinned: false,
    });

    const tabs = useEditorTabStore.getState().resolveGroupTabs('default_editor');
    expect(tabs).toHaveLength(1);
    expect(tabs[0]?.id).toBe('events/B.yssbi-event');
    expect(tabs[0]?.pinned).toBe(false);
  });

  it('pins an existing preview tab when reopened with pinned: true', () => {
    openEditorTab(buildGraphLayoutTab('events/A.yssbi-event', 'event', { pinned: false }), {
      pinned: false,
    });
    openEditorTab(buildGraphLayoutTab('events/A.yssbi-event', 'event'), { pinned: true });

    const tab = useEditorTabStore.getState().resolveGroupTabs('default_editor')[0];
    expect(tab?.pinned).toBe(true);
  });

  it('adds pinned tabs without replacing preview', () => {
    openEditorTab(buildGraphLayoutTab('events/A.yssbi-event', 'event', { pinned: false }), {
      pinned: false,
    });
    openEditorTab(buildGraphLayoutTab('events/B.yssbi-event', 'event'), { pinned: true });

    const tabs = useEditorTabStore.getState().resolveGroupTabs('default_editor');
    expect(tabs).toHaveLength(2);
    expect(tabs.find((t) => t.id === 'events/A.yssbi-event')?.pinned).toBe(false);
    expect(tabs.find((t) => t.id === 'events/B.yssbi-event')?.pinned).toBe(true);
  });
});
