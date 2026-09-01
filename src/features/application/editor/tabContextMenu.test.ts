import type { TFunction } from "i18next";
import { describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  closeTab: vi.fn(async () => true),
  closeOtherTabs: vi.fn(async () => true),
}));

vi.mock("@/features/core/resource", () => ({
  isGraphResourceDirty: vi.fn(() => false),
}));

vi.mock("./dockviewTabProjection", () => ({
  listDockviewGroupTabs: vi.fn(() => []),
}));

vi.mock("./tabCommands", () => ({
  closeTab: mocks.closeTab,
  closeOtherTabs: mocks.closeOtherTabs,
  closeSavedTabsInGroup: vi.fn(async () => true),
  closeAllTabsInGroup: vi.fn(async () => true),
}));

import { buildTabContextMenuSections } from "./tabContextMenu";

describe("tabContextMenu physical target", () => {
  it("forwards the exact panel instance to Close and Close Others", () => {
    const sections = buildTabContextMenuSections(
      {
        panelInstanceId: "panel-duplicate",
        groupId: "group-a",
        tab: {
          id: "events/Main.yssbi-event",
          type: "event",
          component: "GraphEditor",
        },
      },
      ((key: string) => key) as TFunction,
    );
    const items = sections.flatMap((section) => section.items);

    items.find((item) => item.id === "close")?.onClick?.();
    items.find((item) => item.id === "close-others")?.onClick?.();

    expect(mocks.closeTab).toHaveBeenCalledWith("panel-duplicate");
    expect(mocks.closeOtherTabs).toHaveBeenCalledWith("group-a", "panel-duplicate");
  });
});
