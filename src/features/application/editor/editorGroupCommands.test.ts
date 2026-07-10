import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
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
    useLayoutStore.setState({
      nodes: createInitialWorkbenchNodes(),
      activeEditorGroupId: DEFAULT_EDITOR_GROUP_ID,
    });
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

  it('activates the copied graph session after an edge-drop split', async () => {
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
    expect(switchEditorTab).toHaveBeenCalledWith(created, tab);
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
