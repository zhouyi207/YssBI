import { beforeEach, describe, expect, it, vi } from "vitest";

import type { WorkbenchPanelInfo } from "@/modules/workbench/internal/dockview/workbenchRead";
import {
  captureProjectLifecycleState,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";

const lifecycleMocks = vi.hoisted(() => {
  const state = {
    projectInstanceId: "project-a" as string | null,
    ready: true,
    nextPanelId: 0,
    panels: [] as WorkbenchPanelInfo[],
    events: [] as string[],
  };
  return {
    state,
    invalidateForProjectReplacement: vi.fn(),
    runLayoutTransaction: vi.fn(),
    releasePaneState: vi.fn(),
    resetPaneState: vi.fn(),
    resetGraphSession: vi.fn(),
    resetResultAndContext: vi.fn(),
  };
});

vi.mock("@/features/application/project/projectIOStore", () => ({
  useProjectIOStore: {
    getState: () => ({ projectInstanceId: lifecycleMocks.state.projectInstanceId }),
  },
}));

vi.mock("@/modules/workbench/internal/application/workbenchLayoutController", () => ({
  workbenchLayoutController: {
    invalidateForProjectReplacement: lifecycleMocks.invalidateForProjectReplacement,
  },
}));

vi.mock("@/modules/workbench/internal/dockview/workbenchRead", () => ({
  workbenchDockviewRead: {
    get isReady() {
      return lifecycleMocks.state.ready;
    },
  },
}));

vi.mock("@/modules/workbench/internal/dockview/workbenchDockviewInternal", () => ({
  workbenchDockviewRuntime: { control: {} },
  workbenchDockviewInternal: {
    runLayoutTransaction: lifecycleMocks.runLayoutTransaction,
  },
}));

vi.mock("@/modules/workbench/internal/dockview/editorPaneStateStore", () => ({
  useEditorPaneStateStore: {
    getState: () => ({
      release: lifecycleMocks.releasePaneState,
      reset: lifecycleMocks.resetPaneState,
    }),
  },
}));

vi.mock("@/features/core/graphSession/graphSessionStore", () => ({
  useGraphSessionStore: {
    getState: () => ({ reset: lifecycleMocks.resetGraphSession }),
  },
}));

vi.mock("@/features/application/project/projectReset", () => ({
  resetProjectScopedRightSidebarState: lifecycleMocks.resetResultAndContext,
}));

import { removeProjectScopedWorkbenchPanels } from "./projectWorkbenchLifecycle";

function panel(
  panelInstanceId: string,
  metadata: WorkbenchPanelInfo["metadata"],
): WorkbenchPanelInfo {
  const component =
    metadata.role === "editor"
      ? "EditorResource"
      : metadata.role === "result"
        ? "Result"
        : (
            {
              project: "Project",
              nodes: "Nodes",
              data: "Data",
              commands: "Commands",
              details: "Details",
              assistant: "Assistant",
              inspect: "Inspect",
              logs: "Logs",
              output: "Output",
              problems: "Problems",
            } as const
          )[metadata.viewId];
  return {
    panelInstanceId,
    groupId: "group-main",
    component,
    metadata,
    active: false,
    location: { type: "grid" },
  };
}

function openEditor(resourceRef: string): WorkbenchPanelInfo {
  const existing = lifecycleMocks.state.panels.find(
    (candidate) =>
      candidate.metadata.role === "editor" && candidate.metadata.resourceRef === resourceRef,
  );
  if (existing) return existing;
  lifecycleMocks.state.nextPanelId += 1;
  const created = panel(`editor-${lifecycleMocks.state.nextPanelId}`, {
    role: "editor",
    resourceRef,
    resourceKind: "event",
    pinned: true,
  });
  lifecycleMocks.state.panels.push(created);
  return created;
}

function installCommittedTransaction(): void {
  lifecycleMocks.runLayoutTransaction.mockImplementation(async (operation) => {
    lifecycleMocks.state.events.push("transaction:start");
    const removed = new Set<string>();
    const result = operation({
      listPanels: () => [...lifecycleMocks.state.panels],
      removePanels: (panelInstanceIds: readonly string[]) => {
        panelInstanceIds.forEach((panelInstanceId) => removed.add(panelInstanceId));
      },
    });
    lifecycleMocks.state.panels = lifecycleMocks.state.panels.filter(
      (candidate) => !removed.has(candidate.panelInstanceId),
    );
    lifecycleMocks.state.events.push("transaction:committed");
    return result;
  });
}

describe("project workbench lifecycle", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    lifecycleMocks.state.projectInstanceId = "project-a";
    startProjectLifecycle("project-a");
    lifecycleMocks.state.ready = true;
    lifecycleMocks.state.nextPanelId = 0;
    lifecycleMocks.state.panels = [];
    lifecycleMocks.state.events = [];
    lifecycleMocks.invalidateForProjectReplacement.mockImplementation(() => {
      lifecycleMocks.state.events.push("layout:invalidated");
    });
    lifecycleMocks.releasePaneState.mockImplementation((panelInstanceId: string) => {
      lifecycleMocks.state.events.push(`pane:released:${panelInstanceId}`);
    });
    lifecycleMocks.resetPaneState.mockImplementation(() => {
      lifecycleMocks.state.events.push("pane:reset");
    });
    lifecycleMocks.resetGraphSession.mockImplementation(() => {
      lifecycleMocks.state.events.push("session:reset");
    });
    lifecycleMocks.resetResultAndContext.mockImplementation(() => {
      lifecycleMocks.state.events.push("result-context:reset");
    });
    installCommittedTransaction();
  });

  it("removes only project-scoped panels before allowing a same-resource editor to reopen", async () => {
    const resourceRef = "events/Shared.yssbi-event";
    const oldEditor = openEditor(resourceRef);
    lifecycleMocks.state.panels.push(
      panel("result-old", {
        role: "result",
        resultKey: "result-key",
        resultId: "result-old",
        title: "Old result",
        presentation: { kind: "inspector" },
        source: null,
      }),
      panel("details-old", { role: "view", viewId: "details" }),
      panel("inspect-old", { role: "view", viewId: "inspect" }),
      panel("project-stable", { role: "view", viewId: "project" }),
      panel("nodes-stable", { role: "view", viewId: "nodes" }),
      panel("data-stable", { role: "view", viewId: "data" }),
      panel("commands-stable", { role: "view", viewId: "commands" }),
      panel("logs-stable", { role: "view", viewId: "logs" }),
      panel("output-stable", { role: "view", viewId: "output" }),
    );
    const owner = captureProjectLifecycleState();

    await removeProjectScopedWorkbenchPanels("project-a", owner);

    expect(lifecycleMocks.state.panels.map((candidate) => candidate.panelInstanceId)).toEqual([
      "details-old",
      "project-stable",
      "nodes-stable",
      "data-stable",
      "commands-stable",
      "logs-stable",
      "output-stable",
    ]);
    const newEditor = openEditor(resourceRef);
    expect(newEditor.panelInstanceId).not.toBe(oldEditor.panelInstanceId);
    expect(lifecycleMocks.releasePaneState).not.toHaveBeenCalled();
    expect(lifecycleMocks.resetPaneState).toHaveBeenCalledOnce();
    expect(lifecycleMocks.state.events.indexOf("transaction:committed")).toBeLessThan(
      lifecycleMocks.state.events.indexOf("pane:reset"),
    );
    expect(lifecycleMocks.state.events.indexOf("transaction:committed")).toBeLessThan(
      lifecycleMocks.state.events.indexOf("session:reset"),
    );
    expect(lifecycleMocks.state.events.indexOf("transaction:committed")).toBeLessThan(
      lifecycleMocks.state.events.indexOf("result-context:reset"),
    );
  });

  it("resets all pane state without a transaction when the root is unbound", async () => {
    lifecycleMocks.state.ready = false;
    openEditor("events/Shared.yssbi-event");
    const owner = captureProjectLifecycleState();

    await removeProjectScopedWorkbenchPanels("project-a", owner);

    expect(lifecycleMocks.invalidateForProjectReplacement).toHaveBeenCalledOnce();
    expect(lifecycleMocks.runLayoutTransaction).not.toHaveBeenCalled();
    expect(lifecycleMocks.releasePaneState).not.toHaveBeenCalled();
    expect(lifecycleMocks.resetPaneState).toHaveBeenCalledOnce();
    expect(lifecycleMocks.resetGraphSession).toHaveBeenCalledOnce();
    expect(lifecycleMocks.resetResultAndContext).toHaveBeenCalledOnce();
    expect(lifecycleMocks.state.events).toEqual([
      "layout:invalidated",
      "pane:reset",
      "session:reset",
      "result-context:reset",
    ]);
  });

  it("rejects a current-owner cleanup when the ProjectIO identity is unexpected", async () => {
    const owner = captureProjectLifecycleState();
    lifecycleMocks.state.projectInstanceId = "project-b";

    await expect(removeProjectScopedWorkbenchPanels("project-a", owner)).rejects.toMatchObject({
      code: "project_lifecycle_protocol_error",
      zeroEffects: true,
    });

    expect(lifecycleMocks.invalidateForProjectReplacement).not.toHaveBeenCalled();
    expect(lifecycleMocks.runLayoutTransaction).not.toHaveBeenCalled();
    expect(lifecycleMocks.resetPaneState).not.toHaveBeenCalled();
    expect(lifecycleMocks.resetGraphSession).not.toHaveBeenCalled();
    expect(lifecycleMocks.resetResultAndContext).not.toHaveBeenCalled();
  });

  it("rechecks the previous identity inside the FIFO before touching panels", async () => {
    const oldEditor = openEditor("events/Shared.yssbi-event");
    const owner = captureProjectLifecycleState();
    lifecycleMocks.runLayoutTransaction.mockImplementation(async (operation) => {
      lifecycleMocks.state.projectInstanceId = "project-b";
      return operation({
        listPanels: () => [...lifecycleMocks.state.panels],
        removePanels: vi.fn(),
      });
    });

    await expect(removeProjectScopedWorkbenchPanels("project-a", owner)).rejects.toMatchObject({
      code: "project_lifecycle_protocol_error",
      zeroEffects: true,
    });

    expect(lifecycleMocks.state.panels).toContain(oldEditor);
    expect(lifecycleMocks.releasePaneState).not.toHaveBeenCalled();
    expect(lifecycleMocks.resetPaneState).not.toHaveBeenCalled();
    expect(lifecycleMocks.resetGraphSession).not.toHaveBeenCalled();
    expect(lifecycleMocks.resetResultAndContext).not.toHaveBeenCalled();
  });

  it("abandons queued cleanup when its lifecycle token expires before the FIFO callback", async () => {
    const oldEditor = openEditor("events/Shared.yssbi-event");
    const owner = captureProjectLifecycleState();
    const removePanels = vi.fn();
    lifecycleMocks.runLayoutTransaction.mockImplementation(async (operation) => {
      startProjectLifecycle("project-b");
      return operation({
        listPanels: () => [...lifecycleMocks.state.panels],
        removePanels,
      });
    });

    await expect(removeProjectScopedWorkbenchPanels("project-a", owner)).resolves.toBeUndefined();

    expect(lifecycleMocks.state.projectInstanceId).toBe("project-a");
    expect(lifecycleMocks.state.panels).toContain(oldEditor);
    expect(removePanels).not.toHaveBeenCalled();
    expect(lifecycleMocks.releasePaneState).not.toHaveBeenCalled();
    expect(lifecycleMocks.resetPaneState).not.toHaveBeenCalled();
    expect(lifecycleMocks.resetGraphSession).not.toHaveBeenCalled();
    expect(lifecycleMocks.resetResultAndContext).not.toHaveBeenCalled();
  });
});
