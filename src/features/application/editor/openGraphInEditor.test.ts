import { beforeEach, describe, expect, it, vi } from "vitest";

import type { WorkbenchEditorPanelInfo } from "@/features/core/dockview/workbenchRead";
import { ensureEditorViewport } from "@/features/core/viewport";
import { openEditorPanel } from "./openEditorPanel";
import { activateEditorPanelAndSyncSession } from "./activateEditorPanelAndSyncSession";
import { openGraphInEditor } from "./openGraphInEditor";

const openedPanel: WorkbenchEditorPanelInfo = {
  panelInstanceId: "panel-returned",
  groupId: "group-returned",
  component: "EditorResource",
  title: "Main",
  metadata: {
    role: "editor",
    resourceRef: "events/Main.yssbi-event",
    resourceKind: "event",
    pinned: true,
  },
  active: true,
  location: { type: "grid" },
};

vi.mock("@/features/core/viewport", () => ({
  ensureEditorViewport: vi.fn(),
  editorViewportScope: (groupId: string, graphPath: string) => ({ groupId, graphPath }),
}));

vi.mock("./openEditorPanel", () => ({
  openEditorPanel: vi.fn(),
  isEditorOpenRejectionHandled: vi.fn(() => false),
}));

vi.mock("./activateEditorPanelAndSyncSession", () => ({
  activateEditorPanelAndSyncSession: vi.fn(async () => true),
}));

vi.mock("@/features/application/observability/appLogger", () => ({
  logger: { graph: { trace: vi.fn() } },
}));

describe("openGraphInEditor", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(openEditorPanel).mockResolvedValue(openedPanel);
  });

  it("uses the authoritative panel and group returned by the awaited editor open", async () => {
    await expect(
      openGraphInEditor("events/Main.yssbi-event", "Main", "event", "requested-group"),
    ).resolves.toBe(openedPanel);

    expect(openEditorPanel).toHaveBeenCalledWith(
      {
        resourceRef: "events/Main.yssbi-event",
        resourceKind: "event",
        pinned: true,
      },
      {
        targetGroupId: "requested-group",
        insertIndex: undefined,
      },
    );
    expect(ensureEditorViewport).toHaveBeenCalledWith({
      groupId: "group-returned",
      graphPath: "events/Main.yssbi-event",
    });
    expect(activateEditorPanelAndSyncSession).toHaveBeenCalledWith(openedPanel);
    expect(vi.mocked(openEditorPanel).mock.invocationCallOrder[0]).toBeLessThan(
      vi.mocked(ensureEditorViewport).mock.invocationCallOrder[0],
    );
  });
});
