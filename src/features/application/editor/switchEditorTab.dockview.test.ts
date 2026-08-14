import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  activate: vi.fn(),
  getActiveGroupId: vi.fn(),
  listGroups: vi.fn(),
  getFocusedGroupId: vi.fn(),
  findPanelsByResource: vi.fn(),
}));

vi.mock('@/features/core/dockview', () => ({
  editorDockviewPort: {
    activate: mocks.activate,
    getActiveGroupId: mocks.getActiveGroupId,
    listGroups: mocks.listGroups,
    findPanelsByResource: mocks.findPanelsByResource,
  },
}));
vi.mock('@/features/core/editor', () => ({
  useEditorStore: { getState: () => ({ setDetailFocus: vi.fn() }) },
}));
vi.mock('@/features/core/graphSession/graphSessionStore', () => ({
  useGraphSessionStore: {
    getState: () => ({
      getFocusedGroupId: mocks.getFocusedGroupId,
      clearFocusedSession: vi.fn(),
    }),
  },
}));
vi.mock('./graphSessionLifecycle', () => ({
  suspendEditorGroupGraphSession: vi.fn(async () => undefined),
}));
vi.mock('./activateGraphTab', () => ({ activateGraphTab: vi.fn(async () => true) }));
vi.mock('@/features/core/editor/detail/variablesGraphScope', () => ({
  syncVariablesGraphScopeFromActiveTab: vi.fn(),
}));
vi.mock('./ensureDetailVisible', () => ({ ensureDetailVisible: vi.fn() }));

import { focusEditorGroupSync, synchronizeActiveEditorTab } from './switchEditorTab';

describe('focusEditorGroupSync Dockview activation', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.getFocusedGroupId.mockReturnValue('group-a');
    mocks.getActiveGroupId.mockReturnValue('group-a');
    mocks.listGroups.mockReturnValue([{
      groupId: 'group-a',
      activePanelInstanceId: 'panel-a',
      panelInstanceIds: ['panel-a'],
      active: true,
    }]);
    mocks.findPanelsByResource.mockReturnValue([]);
  });

  it('does not reactivate the already active Dockview group', () => {
    expect(focusEditorGroupSync('group-a')).toBe(false);
    expect(mocks.activate).not.toHaveBeenCalled();
  });

  it('synchronizes a Dockview activation without writing activation back to Dockview', async () => {
    mocks.findPanelsByResource.mockReturnValue([{
      panelInstanceId: 'panel-a',
      groupId: 'group-a',
      active: false,
      tab: { resourceRef: 'events/A', kind: 'event' },
    }]);

    await synchronizeActiveEditorTab('group-a', {
      id: 'events/A', type: 'event', component: 'GraphEditor',
    });

    expect(mocks.activate).not.toHaveBeenCalled();
  });

  it('lets only the latest rapid tab switch activate a panel', async () => {
    mocks.findPanelsByResource.mockImplementation((resourceId: string) => [{
      panelInstanceId: resourceId === 'events/A' ? 'panel-a' : 'panel-b',
      groupId: 'group-a',
      active: false,
      tab: { resourceRef: resourceId, kind: 'event' },
    }]);

    const { switchEditorTab } = await import('./switchEditorTab');
    const first = switchEditorTab('group-a', {
      id: 'events/A', type: 'event', component: 'GraphEditor',
    });
    const second = switchEditorTab('group-a', {
      id: 'events/B', type: 'event', component: 'GraphEditor',
    });
    await Promise.all([first, second]);

    expect(mocks.activate).toHaveBeenCalledTimes(1);
    expect(mocks.activate).toHaveBeenCalledWith('panel-b');
  });
});
