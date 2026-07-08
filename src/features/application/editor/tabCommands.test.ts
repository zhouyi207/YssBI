import { describe, expect, it, vi, beforeEach } from 'vitest';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { layoutTabResourceRef } from '@/features/core/layout/layoutTabModel';
import { buildGraphLayoutTab } from '@/features/core/layout/layoutTabModel';
import { switchTab } from './tabCommands';
import { switchEditorTab } from './switchEditorTab';

vi.mock('./switchEditorTab', () => ({
  switchEditorTab: vi.fn().mockResolvedValue(true),
}));

describe('tabCommands', () => {
  beforeEach(() => {
    vi.mocked(switchEditorTab).mockClear();
  });

  it('layoutTabResourceRef maps graph tabs to ResourceRef', () => {
    const tab = buildGraphLayoutTab('events/A.yssbi-event', 'A', 'event');
    expect(layoutTabResourceRef(tab)).toEqual({ id: 'events/A.yssbi-event', kind: 'event' });
  });

  it('switchTab delegates to switchEditorTab', async () => {
    const tab = buildGraphLayoutTab('events/A.yssbi-event', 'A', 'event');
    useLayoutStore.setState((state) => ({
      nodes: {
        ...state.nodes,
        default_editor: {
          ...state.nodes.default_editor,
          data: {
            ...state.nodes.default_editor.data,
            tabs: [tab],
            activeTabId: tab.id,
          },
        },
      },
    }));

    await switchTab('default_editor', tab.id);
    expect(switchEditorTab).toHaveBeenCalledWith('default_editor', tab);
  });
});
