import type { TFunction } from "i18next";
import { describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  requestCloseEditorPanel: vi.fn(async () => true),
  requestCloseOtherEditorPanels: vi.fn(async () => true),
}));

vi.mock("@/features/core/resource", () => ({
  isResourceDocumentDirty: vi.fn(() => false),
}));

vi.mock("@/features/core/dockview", () => ({
  workbenchDockviewRead: { listEditorPanelsInGroup: vi.fn(() => []) },
}));

vi.mock("./editorPanelCloseCommands", () => ({
  requestCloseEditorPanel: mocks.requestCloseEditorPanel,
  requestCloseOtherEditorPanels: mocks.requestCloseOtherEditorPanels,
  requestCloseSavedEditorPanelsInGroup: vi.fn(async () => true),
  requestCloseAllEditorPanelsInGroup: vi.fn(async () => true),
}));

import { buildEditorPanelTabMenu } from "./editorPanelTabMenu";

describe("editor panel tab menu physical target", () => {
  it("forwards the exact panel instance to Close and Close Others", () => {
    const sections = buildEditorPanelTabMenu(
      {
        panelInstanceId: "panel-duplicate",
        groupId: "group-a",
      },
      ((key: string) => key) as TFunction,
    );
    const items = sections.flatMap((section) => section.items);

    items.find((item) => item.id === "close")?.onClick?.();
    items.find((item) => item.id === "close-others")?.onClick?.();

    expect(mocks.requestCloseEditorPanel).toHaveBeenCalledWith("panel-duplicate");
    expect(mocks.requestCloseOtherEditorPanels).toHaveBeenCalledWith("group-a", "panel-duplicate");
  });
});
