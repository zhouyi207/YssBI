import { beforeEach, describe, expect, it } from 'vitest';
import {
  DEFAULT_WORKBENCH_UI_STATE,
  useWorkbenchStore,
} from './workbenchStore';

function uiState() {
  const {
    sidebarCurrentTab,
    sidebarUserHidden,
    panelCollapsed,
    detailUserHidden,
    isSettingsOpen,
    isNodeDocumentationOpen,
    zenMode,
  } = useWorkbenchStore.getState();

  return {
    sidebarCurrentTab,
    sidebarUserHidden,
    panelCollapsed,
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
    expect(uiState()).not.toHaveProperty('pixelSize');
    expect(uiState()).not.toHaveProperty('nodes');
    expect(uiState()).not.toHaveProperty('activeEditorGroupId');
    expect(uiState()).not.toHaveProperty('tabs');
  });

  it('tracks sidebar tab and user visibility intent without layout state', () => {
    const commands = useWorkbenchStore.getState();

    commands.toggleSidebarTab('graphs');
    expect(uiState()).toMatchObject({
      sidebarCurrentTab: 'graphs',
      sidebarUserHidden: true,
    });

    commands.toggleSidebarTab('charts');
    expect(uiState()).toMatchObject({
      sidebarCurrentTab: 'charts',
      sidebarUserHidden: false,
    });

    commands.setSidebarUserHidden(true);
    commands.showSidebarTab('nodes');
    expect(uiState()).toMatchObject({
      sidebarCurrentTab: 'nodes',
      sidebarUserHidden: false,
    });
  });

  it('updates panel, detail, modal, and zen UI state independently', () => {
    const commands = useWorkbenchStore.getState();

    commands.togglePanelCollapsed();
    commands.toggleDetailVisibilityPreference();
    commands.openSettings();
    commands.setNodeDocumentationOpen(true);
    commands.enterZenMode();

    expect(uiState()).toEqual({
      ...DEFAULT_WORKBENCH_UI_STATE,
      panelCollapsed: true,
      detailUserHidden: true,
      isSettingsOpen: true,
      isNodeDocumentationOpen: true,
      zenMode: true,
    });

    commands.exitZenMode();
    expect(uiState()).toMatchObject({
      panelCollapsed: true,
      detailUserHidden: true,
      zenMode: false,
    });
  });

  it('resets only the workbench UI projection', () => {
    const commands = useWorkbenchStore.getState();
    commands.showSidebarTab('variables');
    commands.setPanelCollapsed(true);
    commands.setDetailUserHidden(true);
    commands.setSettingsOpen(true);
    commands.setNodeDocumentationOpen(true);
    commands.toggleZenMode();

    commands.resetWorkbenchUIState();

    expect(uiState()).toEqual(DEFAULT_WORKBENCH_UI_STATE);
  });
});
