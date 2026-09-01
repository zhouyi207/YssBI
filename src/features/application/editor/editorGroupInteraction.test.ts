import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  focusEditorGroupSync: vi.fn(),
  hydrateEditorGroup: vi.fn(),
}));

vi.mock("./activateEditorPanelAndSyncSession", () => ({
  focusEditorGroupSync: mocks.focusEditorGroupSync,
  hydrateEditorGroup: mocks.hydrateEditorGroup,
}));

import { prepareEditorGroupForInteraction } from "./editorGroupInteraction";

describe("editor group interaction preparation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("does not rehydrate an already focused group", () => {
    mocks.focusEditorGroupSync.mockReturnValue(false);

    prepareEditorGroupForInteraction("group-a");

    expect(mocks.focusEditorGroupSync).toHaveBeenCalledWith("group-a");
    expect(mocks.hydrateEditorGroup).not.toHaveBeenCalled();
  });
});
