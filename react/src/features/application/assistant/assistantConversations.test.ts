import { expect, it, vi } from "vitest";
import { HarnessService, type HarnessSession } from "@/services/assistant/harnessService";
import { getActivityPanelDocument } from "@/services/workbench/activityPanelService";
import { useSidebarStore } from "@/features/core/sidebar/sidebarStore";
import {
  captureProjectLifecycleState,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { activityPanelFixture } from "@/tests/helpers/activityPanelFixture";
import type { ActivityPanelSnapshot } from "@/shared/types/domain/activityPanel";
import {
  createAssistantConversation,
  reloadAssistantConversations,
  renameAssistantConversation,
} from "./assistantConversations";

const project = vi.hoisted(() => ({ projectInstanceId: "project-a" }));
vi.mock("@/features/application/project/projectIOStore", () => ({
  useProjectIOStore: { getState: () => project },
}));
vi.mock("@/features/application/editorMutation/projectPublicationCoordinator", () => ({
  projectPublicationCoordinator: { refreshIndex: vi.fn() },
}));
vi.mock("@/services/workbench/activityPanelService", () => ({ getActivityPanelDocument: vi.fn() }));
vi.mock("@/services/assistant/harnessService", () => ({
  HarnessService: { createSession: vi.fn(), renameSession: vi.fn(), subscribeEvents: vi.fn() },
}));

function bind(projectInstanceId: string) {
  project.projectInstanceId = projectInstanceId;
  startProjectLifecycle(projectInstanceId);
  useSidebarStore.getState().clearProjectPanels();
  return useSidebarStore.getState().bindPanel({
    ...captureProjectLifecycleState(),
    panelId: "assistant",
    locale: "en-US",
  });
}
function snapshot(projectInstanceId: string, title: string): ActivityPanelSnapshot {
  return {
    cursor: title,
    document: {
      ...activityPanelFixture("assistant", [
        {
          kind: "item",
          id: "conversation:session",
          depth: 0,
          item: { kind: "conversation", sessionId: "session", title, lastOpenedAt: 1 },
        },
      ]),
      projectInstanceId,
    },
  };
}

it("loads only the Activity document and ignores a previous project's delayed list", async () => {
  let resolvePrevious!: (value: ActivityPanelSnapshot) => void;
  vi.mocked(getActivityPanelDocument)
    .mockReset()
    .mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolvePrevious = resolve;
        }),
    )
    .mockResolvedValueOnce(snapshot("project-b", "B"));
  bind("project-a");
  const previous = reloadAssistantConversations();
  bind("project-b");
  await reloadAssistantConversations();
  resolvePrevious(snapshot("project-a", "A"));
  await previous;
  expect(useSidebarStore.getState().panels.assistant).toMatchObject({
    binding: { projectInstanceId: "project-b" },
    snapshot: snapshot("project-b", "B"),
    loading: false,
  });
  expect(HarnessService.createSession).not.toHaveBeenCalled();
  expect(HarnessService.subscribeEvents).not.toHaveBeenCalled();
});

it("refreshes committed mutations from Rust and discards mutation completion after project replacement", async () => {
  bind("project-a");
  const session: HarnessSession = {
    sessionId: "session",
    title: "mutation reply",
    lastOpenedAt: 1,
    projectInstanceId: "project-a",
    projectSessionId: "binding",
    model: null,
  };
  vi.mocked(HarnessService.renameSession).mockResolvedValue(session);
  vi.mocked(getActivityPanelDocument)
    .mockReset()
    .mockResolvedValue(snapshot("project-a", "Rust title"));
  await renameAssistantConversation("session", "  Renamed  ");
  expect(HarnessService.renameSession).toHaveBeenCalledWith("session", "Renamed");
  expect(useSidebarStore.getState().panels.assistant?.snapshot).toEqual(
    snapshot("project-a", "Rust title"),
  );
  let resolveCreate!: (value: HarnessSession) => void;
  vi.mocked(HarnessService.createSession).mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        resolveCreate = resolve;
      }),
  );
  const creating = createAssistantConversation();
  bind("project-b");
  resolveCreate(session);
  expect(await creating).toBeNull();
  expect(getActivityPanelDocument).toHaveBeenCalledOnce();
  expect(useSidebarStore.getState().panels.assistant?.snapshot).toBeNull();
});
