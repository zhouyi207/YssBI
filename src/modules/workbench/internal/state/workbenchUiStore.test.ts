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
    useWorkbenchUiStore.setState(DEFAULT_WORKBENCH_UI_STATE);
  });

  it("opens and closes node documentation", () => {
    const commands = useWorkbenchUiStore.getState();

    commands.setNodeDocumentationOpen(true);

    expect(uiState()).toEqual({
      isNodeDocumentationOpen: true,
    });

    commands.setNodeDocumentationOpen(false);

    expect(uiState()).toEqual(DEFAULT_WORKBENCH_UI_STATE);
  });
});
