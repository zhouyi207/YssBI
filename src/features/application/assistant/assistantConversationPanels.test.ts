import { beforeEach, expect, it, vi } from "vitest";
import {
  openAssistantConversation,
  closeAssistantConversation,
  toggleAssistantConversationWindow,
} from "./assistantConversationPanels";
import { HarnessService } from "@/services/assistant/harnessService";
import { getActivityPanelDocument } from "@/services/workbench/activityPanelService";
import type { ActivityPanelSnapshot } from "@/shared/types/domain/activityPanel";
import { useSidebarStore } from "@/features/core/sidebar/sidebarStore";
import {
  captureProjectLifecycleState,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { activityPanelFixture } from "@/tests/helpers/activityPanelFixture";

const state = vi.hoisted(() => ({
  projectInstanceId: "current-project",
  panels: [] as {
    panelInstanceId: string;
    visible: boolean;
    metadata: { role: "conversation"; sessionId: string } | { role: "view"; viewId: "project" };
  }[],
  open: vi.fn(async () => {}),
  close: vi.fn(async () => true),
  closeAll: vi.fn(async (_ids: readonly string[]) => true),
}));
vi.mock("i18next", () => ({ default: { t: (key: string) => key } }));
vi.mock("@/modules/workbench/public", () => ({
  workbenchLayoutRead: { listPanels: () => state.panels },
  workbenchLayoutControl: { openConversation: state.open },
  showWorkbenchLayoutError: vi.fn(),
  revealWorkbenchView: vi.fn(),
}));
vi.mock("@/features/application/editor/workbenchPanelClose", () => ({
  requestCloseWorkbenchPanel: state.close,
  requestCloseWorkbenchPanels: state.closeAll,
}));
vi.mock("@/features/application/project/projectIOStore", () => ({
  useProjectIOStore: { getState: () => state },
}));
vi.mock("@/features/application/editorMutation/projectPublicationCoordinator", () => ({
  projectPublicationCoordinator: { refreshIndex: vi.fn() },
}));
vi.mock("@/services/workbench/activityPanelService", () => ({ getActivityPanelDocument: vi.fn() }));
vi.mock("@/services/assistant/harnessService", () => ({
  HarnessService: { createSession: vi.fn() },
}));

beforeEach(() => {
  vi.clearAllMocks();
  state.panels = [];
  state.projectInstanceId = "current-project";
  startProjectLifecycle(state.projectInstanceId);
  useSidebarStore.getState().clearProjectPanels();
});

function directory(sessionIds: string[]): ActivityPanelSnapshot {
  return {
    cursor: "directory",
    document: {
      ...activityPanelFixture(
        "assistant",
        sessionIds.map((sessionId) => ({
          id: `conversation:${sessionId}`,
          depth: 0,
          kind: "item",
          item: { kind: "conversation", sessionId, title: sessionId, lastOpenedAt: 1 },
        })),
      ),
      projectInstanceId: state.projectInstanceId,
    },
  };
}

it("opens historical conversations from the current document and toggles only the visible conversation", async () => {
  startProjectLifecycle(state.projectInstanceId);
  const binding = useSidebarStore.getState().bindPanel({
    ...captureProjectLifecycleState(),
    panelId: "assistant",
    locale: "en-US",
  });
  useSidebarStore.getState().publishPanels([
    {
      binding,
      snapshot: {
        cursor: "directory",
        document: {
          ...activityPanelFixture("assistant", [
            {
              id: "conversation:history",
              depth: 0,
              kind: "item",
              item: {
                kind: "conversation",
                sessionId: "history",
                title: "Previous analysis",
                lastOpenedAt: 0,
              },
            },
          ]),
          projectInstanceId: state.projectInstanceId,
        },
      },
    },
  ]);
  await openAssistantConversation("history", true);
  expect(state.open).toHaveBeenCalledWith({ sessionId: "history", title: "Previous analysis" });
  state.panels = [
    {
      panelInstanceId: "chat-panel",
      visible: true,
      metadata: { role: "conversation", sessionId: "history" },
    },
  ];
  await openAssistantConversation("history", true);
  expect(state.close).toHaveBeenCalledWith("chat-panel");
  expect(state.open).toHaveBeenCalledTimes(1);
  state.panels[0].visible = false;
  await openAssistantConversation("history", true);
  expect(state.open).toHaveBeenCalledTimes(2);
  await openAssistantConversation("missing", true);
  state.projectInstanceId = "another-project";
  await openAssistantConversation("history", true);
  expect(state.open).toHaveBeenCalledTimes(2);
  expect(state.close).toHaveBeenCalledTimes(1);
  state.projectInstanceId = "current-project";
  useSidebarStore.getState().clearProjectPanels();
  await closeAssistantConversation("history");
  expect(state.close).toHaveBeenCalledTimes(2);
  expect(state.close).toHaveBeenLastCalledWith("chat-panel");
  expect(state.open).toHaveBeenCalledTimes(2);
});

it("closes the whole conversation window and reopens the previously visible conversation", async () => {
  state.panels = [
    {
      panelInstanceId: "first",
      visible: false,
      metadata: { role: "conversation", sessionId: "first" },
    },
    {
      panelInstanceId: "selected",
      visible: true,
      metadata: { role: "conversation", sessionId: "selected" },
    },
    { panelInstanceId: "project", visible: true, metadata: { role: "view", viewId: "project" } },
  ];
  state.closeAll.mockImplementationOnce(async (ids) => {
    state.panels = state.panels.filter((panel) => !ids.includes(panel.panelInstanceId));
    return true;
  });
  await toggleAssistantConversationWindow();
  expect(state.closeAll).toHaveBeenCalledWith(["first", "selected"]);
  expect(getActivityPanelDocument).not.toHaveBeenCalled();
  expect(state.panels.map((panel) => panel.panelInstanceId)).toEqual(["project"]);
  vi.mocked(getActivityPanelDocument).mockResolvedValueOnce(directory(["first", "selected"]));
  await toggleAssistantConversationWindow();
  expect(state.open).toHaveBeenCalledWith({ sessionId: "selected", title: "selected" });
  expect(HarnessService.createSession).not.toHaveBeenCalled();
});

it("ignores an old project's opening request and creates a conversation only for an empty current directory", async () => {
  let finish!: (snapshot: ActivityPanelSnapshot) => void;
  const previous = directory([]);
  vi.mocked(getActivityPanelDocument).mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  const opening = toggleAssistantConversationWindow();
  state.projectInstanceId = "new-project";
  startProjectLifecycle(state.projectInstanceId);
  useSidebarStore.getState().clearProjectPanels();
  finish(previous);
  await opening;
  expect(state.open).not.toHaveBeenCalled();
  expect(HarnessService.createSession).not.toHaveBeenCalled();
  vi.mocked(getActivityPanelDocument)
    .mockResolvedValueOnce(directory([]))
    .mockResolvedValueOnce(directory(["new-conversation"]));
  vi.mocked(HarnessService.createSession).mockResolvedValueOnce({
    sessionId: "new-conversation",
    title: "new-conversation",
    lastOpenedAt: 1,
    projectInstanceId: state.projectInstanceId,
    projectSessionId: "binding",
    model: null,
  });
  await toggleAssistantConversationWindow();
  expect(HarnessService.createSession).toHaveBeenCalledOnce();
  expect(state.open).toHaveBeenCalledWith({
    sessionId: "new-conversation",
    title: "new-conversation",
  });
});
