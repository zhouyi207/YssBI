import { beforeEach, describe, expect, it } from "vitest";
import { DEFAULT_WORKBENCH_UI_STATE, useWorkbenchUiStore } from "./workbenchUiStore";

function uiState() {
  const { isNodeDocumentationOpen } = useWorkbenchUiStore.getState();

  return {
    isNodeDocumentationOpen,
  };
}

describe("workbenchUiStore", () => {
  beforeEach(() => {
    useWorkbenchUiStore.getState().resetWorkbenchUiState();
  });

  it("updates modal state, then resets it", () => {
    const commands = useWorkbenchUiStore.getState();

    commands.setNodeDocumentationOpen(true);

    expect(uiState()).toEqual({
      isNodeDocumentationOpen: true,
    });

    commands.resetWorkbenchUiState();

    expect(uiState()).toEqual(DEFAULT_WORKBENCH_UI_STATE);
  });
});
