import { beforeEach, describe, expect, it } from 'vitest';
import {
  DEFAULT_WORKBENCH_UI_STATE,
  useWorkbenchStore,
} from './workbenchStore';

function uiState() {
  const {
    isSettingsOpen,
    isNodeDocumentationOpen,
  } = useWorkbenchStore.getState();

  return {
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

    expect(state).not.toHaveProperty('sidebarUserHidden');
    expect(state).not.toHaveProperty('detailUserHidden');
    expect(state).not.toHaveProperty('panelCollapsed');
    expect(state).not.toHaveProperty('zenMode');
    expect(state).not.toHaveProperty('activeEditorGroupId');
    expect(state).not.toHaveProperty('tabs');
  });

  it('updates modal state, then resets it', () => {
    const commands = useWorkbenchStore.getState();

    commands.openSettings();
    commands.setNodeDocumentationOpen(true);

    expect(uiState()).toEqual({
      isSettingsOpen: true,
      isNodeDocumentationOpen: true,
    });

    commands.closeSettings();
    commands.resetWorkbenchUIState();

    expect(uiState()).toEqual(DEFAULT_WORKBENCH_UI_STATE);
  });
});
