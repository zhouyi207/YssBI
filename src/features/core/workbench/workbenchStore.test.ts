import { beforeEach, describe, expect, it } from 'vitest';
import {
  DEFAULT_WORKBENCH_UI_STATE,
  useWorkbenchStore,
} from './workbenchStore';

function uiState() {
  const {
    sidebarCurrentTab,
    sidebarUserHidden,
    detailUserHidden,
    isSettingsOpen,
    isNodeDocumentationOpen,
    zenMode,
  } = useWorkbenchStore.getState();

  return {
    sidebarCurrentTab,
    sidebarUserHidden,
    detailUserHidden,
    isSettingsOpen,
    isNodeDocumentationOpen,
    zenMode,
  };
}

describe('workbenchStore', () => {
  beforeEach(() => {
    useWorkbenchStore.getState().resetWorkbenchUIState();
  });

  it('starts from the non-layout workbench defaults', () => {
    expect(uiState()).toEqual(DEFAULT_WORKBENCH_UI_STATE);
    expect(useWorkbenchStore.getState()).not.toHaveProperty('panelCollapsed');
    expect(uiState()).not.toHaveProperty('pixelSize');
    expect(uiState()).not.toHaveProperty('nodes');
    expect(uiState()).not.toHaveProperty('activeEditorGroupId');
    expect(uiState()).not.toHaveProperty('tabs');
  });

  it('tracks sidebar tab and user visibility intent without layout state', () => {
    const commands = useWorkbenchStore.getState();

    commands.toggleSidebarTab('project');
    expect(uiState()).toMatchObject({
      sidebarCurrentTab: 'project',
      sidebarUserHidden: true,
    });

    commands.toggleSidebarTab('commands');
    expect(uiState()).toMatchObject({
      sidebarCurrentTab: 'commands',
      sidebarUserHidden: false,
    });

    commands.setSidebarUserHidden(true);
    commands.showSidebarTab('nodes');
    expect(uiState()).toMatchObject({
      sidebarCurrentTab: 'nodes',
      sidebarUserHidden: false,
    });
  });

  it('updates detail, modal, and zen UI state independently', () => {
    const commands = useWorkbenchStore.getState();

    commands.toggleDetailVisibilityPreference();
    commands.openSettings();
    commands.setNodeDocumentationOpen(true);
    commands.enterZenMode();

    expect(uiState()).toEqual({
      ...DEFAULT_WORKBENCH_UI_STATE,
      detailUserHidden: true,
      isSettingsOpen: true,
      isNodeDocumentationOpen: true,
      zenMode: true,
    });

    commands.exitZenMode();
    expect(uiState()).toMatchObject({
      detailUserHidden: true,
      zenMode: false,
    });
  });

  it('resets only the workbench UI projection', () => {
    const commands = useWorkbenchStore.getState();
    commands.showSidebarTab('data');
    commands.setDetailUserHidden(true);
    commands.setSettingsOpen(true);
    commands.setNodeDocumentationOpen(true);
    commands.toggleZenMode();

    commands.resetWorkbenchUIState();

    expect(uiState()).toEqual(DEFAULT_WORKBENCH_UI_STATE);
  });
});
