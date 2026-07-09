import { beforeEach, describe, expect, it } from 'vitest';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { buildGraphLayoutTab } from '@/features/core/layout/layoutTabModel';
import { openEditorTab } from './openEditorTab';

describe('openEditorTab preview', () => {
  beforeEach(() => {
    useLayoutStore.setState({
      nodes: {
        default_editor: {
          id: 'default_editor',
          type: 'component',
          parentId: 'center',
          data: {
            component: 'GraphEditor',
            tabs: [],
            activeTabId: undefined,
          },
        },
      },
      activeEditorGroupId: 'default_editor',
    } as Partial<ReturnType<typeof useLayoutStore.getState>>);
  });

  it('replaces the preview tab when opening another preview in the same group', () => {
    openEditorTab(buildGraphLayoutTab('events/A.yssbi-event', 'A', 'event', { pinned: false }), {
      pinned: false,
    });
    openEditorTab(buildGraphLayoutTab('events/B.yssbi-event', 'B', 'event', { pinned: false }), {
      pinned: false,
    });

    const tabs = useLayoutStore.getState().nodes.default_editor.data?.tabs ?? [];
    expect(tabs).toHaveLength(1);
    expect(tabs[0]?.id).toBe('events/B.yssbi-event');
    expect(tabs[0]?.pinned).toBe(false);
  });

  it('pins an existing preview tab when reopened with pinned: true', () => {
    openEditorTab(buildGraphLayoutTab('events/A.yssbi-event', 'A', 'event', { pinned: false }), {
      pinned: false,
    });
    openEditorTab(buildGraphLayoutTab('events/A.yssbi-event', 'A', 'event'), { pinned: true });

    const tab = useLayoutStore.getState().nodes.default_editor.data?.tabs?.[0];
    expect(tab?.pinned).toBe(true);
  });

  it('adds pinned tabs without replacing preview', () => {
    openEditorTab(buildGraphLayoutTab('events/A.yssbi-event', 'A', 'event', { pinned: false }), {
      pinned: false,
    });
    openEditorTab(buildGraphLayoutTab('events/B.yssbi-event', 'B', 'event'), { pinned: true });

    const tabs = useLayoutStore.getState().nodes.default_editor.data?.tabs ?? [];
    expect(tabs).toHaveLength(2);
    expect(tabs.find((t) => t.id === 'events/A.yssbi-event')?.pinned).toBe(false);
    expect(tabs.find((t) => t.id === 'events/B.yssbi-event')?.pinned).toBe(true);
  });
});
