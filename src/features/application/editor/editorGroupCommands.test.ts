import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { useEditorTabStore } from '@/features/core/layout/editorTabStore';
import {
  createInitialWorkbenchNodes,
  DEFAULT_EDITOR_GROUP_ID,
} from '@/features/core/layout/workbenchLayoutDefaults';
import {
  moveTabBetweenGroups,
  splitEditorAtEdge,
  splitEditorWithTab,
} from './editorGroupCommands';
import { switchEditorTab } from './switchEditorTab';

vi.mock('./switchEditorTab', () => ({
  activateEditorGroup: vi.fn().mockResolvedValue(true),
  switchEditorTab: vi.fn().mockResolvedValue(true),
}));

describe('editorGroupCommands', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useEditorTabStore.setState({ registry: {}, placements: {} });
    useLayoutStore.setState({
      nodes: createInitialWorkbenchNodes(),
      activeEditorGroupId: DEFAULT_EDITOR_GROUP_ID,
    });
    useEditorTabStore.getState().ensureGroupPlacement(DEFAULT_EDITOR_GROUP_ID);
  });

  it('activates the moved tab session in the target group', () => {
    const tab = {
      id: 'events/A.yssbi-event',
      component: 'GraphEditor' as const,
      type: 'event' as const,
    };
    useLayoutStore.getState().addTab(DEFAULT_EDITOR_GROUP_ID, tab);
    const targetGroupId = useLayoutStore.getState().splitEditorGroupAtEdge(
      DEFAULT_EDITOR_GROUP_ID,
      'right',
      { component: 'GraphEditor', tabs: [] },
    );
    expect(targetGroupId).toBeTruthy();

    moveTabBetweenGroups(DEFAULT_EDITOR_GROUP_ID, tab.id, targetGroupId!);

    expect(switchEditorTab).toHaveBeenCalledWith(targetGroupId, { ...tab, pinned: true });
  });

  it('removes an empty source group after moving its last tab', () => {
    const tab = {
      id: 'events/A.yssbi-event',
      component: 'GraphEditor' as const,
      type: 'event' as const,
    };
    useLayoutStore.getState().addTab(DEFAULT_EDITOR_GROUP_ID, tab);
    const targetGroupId = useLayoutStore.getState().splitEditorGroupAtEdge(
      DEFAULT_EDITOR_GROUP_ID,
      'right',
      { component: 'GraphEditor', tabs: [] },
    );
    expect(targetGroupId).toBeTruthy();

    moveTabBetweenGroups(DEFAULT_EDITOR_GROUP_ID, tab.id, targetGroupId!);

    const layout = useLayoutStore.getState();
    expect(layout.nodes[DEFAULT_EDITOR_GROUP_ID]).toBeUndefined();
    expect(layout.activeEditorGroupId).toBe(targetGroupId);
    expect(useEditorTabStore.getState().getPlacement(targetGroupId!).tabIds).toEqual([tab.id]);
  });

  it('copies the only tab when edge-drop splitting a single-tab group', async () => {
    const tab = {
      id: 'events/A.yssbi-event',
      component: 'GraphEditor' as const,
      type: 'event' as const,
    };
    useLayoutStore.getState().addTab(DEFAULT_EDITOR_GROUP_ID, tab);

    const created = await splitEditorWithTab(
      DEFAULT_EDITOR_GROUP_ID,
      tab.id,
      DEFAULT_EDITOR_GROUP_ID,
      'bottom',
    );

    expect(created).toBeTruthy();
    expect(switchEditorTab).toHaveBeenCalledWith(created, { ...tab, pinned: true });
    expect(
      useEditorTabStore.getState().getPlacement(DEFAULT_EDITOR_GROUP_ID).tabIds.includes(tab.id),
    ).toBe(true);
    expect(
      useEditorTabStore.getState().getPlacement(created!).tabIds.includes(tab.id),
    ).toBe(true);
  });

  it('moves only the dragged tab when the source group has siblings', async () => {
    const tabA = {
      id: 'events/A.yssbi-event',
      component: 'GraphEditor' as const,
      type: 'event' as const,
    };
    const tabB = {
      id: 'events/B.yssbi-event',
      component: 'GraphEditor' as const,
      type: 'event' as const,
    };
    useLayoutStore.getState().addTab(DEFAULT_EDITOR_GROUP_ID, tabA);
    useLayoutStore.getState().addTab(DEFAULT_EDITOR_GROUP_ID, tabB);

    const created = await splitEditorWithTab(
      DEFAULT_EDITOR_GROUP_ID,
      tabB.id,
      DEFAULT_EDITOR_GROUP_ID,
      'right',
    );

    expect(created).toBeTruthy();
    const sourceTabIds = useEditorTabStore.getState().getPlacement(DEFAULT_EDITOR_GROUP_ID).tabIds;
    expect(sourceTabIds).toEqual([tabA.id]);
    expect(
      useEditorTabStore.getState().getPlacement(created!).tabIds,
    ).toEqual([tabB.id]);
  });

  it('returns and activates the created group after a command split', async () => {
    const tab = {
      id: 'events/A.yssbi-event',
      component: 'GraphEditor' as const,
      type: 'event' as const,
    };
    useLayoutStore.getState().addTab(DEFAULT_EDITOR_GROUP_ID, tab);

    const created = await splitEditorAtEdge(DEFAULT_EDITOR_GROUP_ID, 'right');

    expect(created).toBeTruthy();
    expect(switchEditorTab).toHaveBeenCalledWith(created, { ...tab, pinned: true });
  });
});
