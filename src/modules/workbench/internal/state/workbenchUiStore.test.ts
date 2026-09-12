import { beforeEach, describe, expect, it } from "vitest";
import { DEFAULT_WORKBENCH_UI_STATE, useWorkbenchUiStore } from "./workbenchUiStore";

function uiState() {
  const { isSettingsOpen, isNodeDocumentationOpen } = useWorkbenchUiStore.getState();

  return {
    isSettingsOpen,
    isNodeDocumentationOpen,
  };
}

describe("workbenchUiStore", () => {
  beforeEach(() => {
    useWorkbenchUiStore.getState().resetWorkbenchUiState();
  });

  it("updates modal state, then resets it", () => {
    const commands = useWorkbenchUiStore.getState();

    commands.openSettings();
    commands.setNodeDocumentationOpen(true);

    expect(uiState()).toEqual({
      isSettingsOpen: true,
      isNodeDocumentationOpen: true,
    });

    commands.closeSettings();
    commands.resetWorkbenchUiState();

    expect(uiState()).toEqual(DEFAULT_WORKBENCH_UI_STATE);
  });
});
