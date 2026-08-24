import { beforeEach, describe, expect, it } from 'vitest';
import {
  DEFAULT_WORKBENCH_UI_STATE,
  useWorkbenchStore,
} from './workbenchStore';

function uiState() {
  const {
    sidebarCurrentTab,
    isSettingsOpen,
    isNodeDocumentationOpen,
  } = useWorkbenchStore.getState();

  return {
    sidebarCurrentTab,
    isSettingsOpen,
    isNodeDocumentationOpen,
  };
}

describe('workbenchStore', () => {
  beforeEach(() => {
    useWorkbenchStore.getState().resetWorkbenchUIState();
  });

  it('keeps only non-placement workbench UI state', () => {
    const state = useWorkbenchStore.getState();

    expect(uiState()).toEqual(DEFAULT_WORKBENCH_UI_STATE);
    expect(state.sidebarCurrentTab).toBe('project');
    expect(state).not.toHaveProperty('sidebarUserHidden');
    expect(state).not.toHaveProperty('detailUserHidden');
    expect(state).not.toHaveProperty('panelCollapsed');
    expect(state).not.toHaveProperty('zenMode');
    expect(state).not.toHaveProperty('activeEditorGroupId');
    expect(state).not.toHaveProperty('tabs');
  });

  it('updates sidebar and modal state, then resets it', () => {
    const commands = useWorkbenchStore.getState();

    commands.setSidebarCurrentTab('nodes');
    commands.openSettings();
    commands.setNodeDocumentationOpen(true);

    expect(uiState()).toEqual({
      sidebarCurrentTab: 'nodes',
      isSettingsOpen: true,
      isNodeDocumentationOpen: true,
    });

    commands.closeSettings();
    commands.resetWorkbenchUIState();

    expect(uiState()).toEqual(DEFAULT_WORKBENCH_UI_STATE);
  });
});
