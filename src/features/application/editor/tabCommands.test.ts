import { describe, expect, it, vi, beforeEach } from 'vitest';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { layoutTabResourceRef } from '@/features/core/layout/layoutTabModel';
import { buildGraphLayoutTab } from '@/features/core/layout/layoutTabModel';
import {
  createInitialWorkbenchNodes,
  DEFAULT_EDITOR_GROUP_ID,
} from '@/features/core/layout/workbenchLayoutDefaults';
import { closeEditorGroup, closeTab, switchTab } from './tabCommands';
import { activateEditorGroup, switchEditorTab } from './switchEditorTab';
import { closeEditorTab } from './closeEditorTab';

vi.mock('./switchEditorTab', () => ({
  switchEditorTab: vi.fn().mockResolvedValue(true),
  activateEditorGroup: vi.fn().mockResolvedValue(true),
}));

vi.mock('./closeEditorTab', () => ({
  closeEditorTab: vi.fn().mockResolvedValue(true),
}));

describe('tabCommands', () => {
  beforeEach(() => {
    vi.clearAllMocks();
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

  it('activates an inactive group before running its TabBar close action', async () => {
    useLayoutStore.setState((state) => ({
      activeEditorGroupId: 'other-editor',
      nodes: {
        ...state.nodes,
        default_editor: {
          ...state.nodes.default_editor,
          data: {
            ...state.nodes.default_editor.data,
            tabs: [{ id: 'events/A.yssbi-event', component: 'GraphEditor', type: 'event' }],
            activeTabId: 'events/A.yssbi-event',
          },
        },
      },
    }));

    await closeTab('default_editor', 'events/A.yssbi-event');

    expect(activateEditorGroup).toHaveBeenCalledWith('default_editor');
    expect(closeEditorTab).toHaveBeenCalledWith('events/A.yssbi-event', 'default_editor', false);
    expect(vi.mocked(activateEditorGroup).mock.invocationCallOrder[0])
      .toBeLessThan(vi.mocked(closeEditorTab).mock.invocationCallOrder[0]);
  });

  it('does not attempt group removal again after the last tab already removed it', async () => {
    useLayoutStore.setState({
      nodes: createInitialWorkbenchNodes(),
      activeEditorGroupId: DEFAULT_EDITOR_GROUP_ID,
    });
    const groupId = useLayoutStore.getState().splitEditorGroupAtEdge(
      DEFAULT_EDITOR_GROUP_ID,
      'right',
      {
        component: 'GraphEditor',
        tabs: [{ id: 'events/B.yssbi-event', component: 'GraphEditor', type: 'event' }],
        activeTabId: 'events/B.yssbi-event',
      },
    );
    expect(groupId).toBeTruthy();

    vi.mocked(closeEditorTab).mockImplementationOnce(async (tabId, targetGroupId) => {
      useLayoutStore.getState().removeTab(targetGroupId!, tabId);
      return true;
    });
    const originalRemoveEditorGroup = useLayoutStore.getState().removeEditorGroup;
    const redundantRemoval = vi.fn(() => false);
    useLayoutStore.setState({ removeEditorGroup: redundantRemoval });

    await closeEditorGroup(groupId!);

    expect(useLayoutStore.getState().nodes[groupId!]).toBeUndefined();
    expect(redundantRemoval).not.toHaveBeenCalled();
    useLayoutStore.setState({ removeEditorGroup: originalRemoveEditorGroup });
  });
});
