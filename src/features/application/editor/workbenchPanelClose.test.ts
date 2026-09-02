import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  EditorResourceKind,
  WorkbenchPanelMetadata,
  WorkbenchViewId,
} from "@/modules/workbench/internal/dockview/workbenchPanelModel";
import type {
  WorkbenchPanelCommitToken,
  WorkbenchPanelInfo,
} from "@/modules/workbench/internal/dockview/workbenchTypes";

const mocks = vi.hoisted(() => {
  type FakePanel = {
    panelInstanceId: string;
    groupId: string;
    metadata: unknown;
  };
  type Token = {
    panelInstanceId: string;
    groupId: string;
    metadata: unknown;
  };

  const panels: FakePanel[] = [];
  const dirty = new Set<string>();
  const project = { projectInstanceId: "project-a", epoch: 1, available: true };
  const chartState = {
    index: [] as Array<Record<string, unknown>>,
    documents: {} as Record<string, unknown>,
  };
  let fifo: Promise<unknown> = Promise.resolve();

  const dirtyKey = (ref: { id: string; kind: string }) => `${ref.kind}\0${ref.id}`;
  const enqueue = (operation: () => unknown | Promise<unknown>): Promise<unknown> => {
    const result = fifo.then(operation);
    fifo = result.then(
      () => undefined,
      () => undefined,
    );
    return result;
  };

  const confirm3 = vi.fn();
  const saveGraphDraft = vi.fn();
  const saveChart = vi.fn();
  const clearResourceDocumentState = vi.fn();
  const showBlockingIpcError = vi.fn();
  const showBlockingMessage = vi.fn();
  const releasePane = vi.fn();
  const releaseEditorViewport = vi.fn();
  const clearDetailFocusForClosedPanel = vi.fn();
  const deactivateGraphPanelSession = vi.fn();
  const unloadGraphDocument = vi.fn();
  const resolveResourceDisplayName = vi.fn();
  const applyChartState = (update: unknown) => {
    const patch =
      typeof update === "function"
        ? (update as (state: typeof chartState) => Partial<typeof chartState>)(chartState)
        : (update as Partial<typeof chartState>);
    Object.assign(chartState, patch);
  };
  const setChartState = vi.fn(applyChartState);

  const isAuthorized = (authorize?: () => boolean): boolean => {
    if (!authorize) return true;
    try {
      return authorize();
    } catch {
      return false;
    }
  };

  const commitRemove = vi.fn((tokens: readonly Token[], authorize?: () => boolean) =>
    enqueue(() => {
      if (!isAuthorized(authorize)) return "stale" as const;
      const currentById = new Map(panels.map((panel) => [panel.panelInstanceId, panel]));
      const stale = tokens.some((token) => {
        const current = currentById.get(token.panelInstanceId);
        return (
          !current ||
          current.groupId !== token.groupId ||
          JSON.stringify(current.metadata) !== JSON.stringify(token.metadata)
        );
      });
      if (stale) return "stale" as const;

      const ids = new Set(tokens.map((token) => token.panelInstanceId));
      for (let index = panels.length - 1; index >= 0; index -= 1) {
        if (ids.has(panels[index].panelInstanceId)) panels.splice(index, 1);
      }
      return "committed" as const;
    }),
  );

  const runPublicationTransaction = vi.fn((operation: () => unknown | Promise<unknown>) =>
    enqueue(operation),
  );

  const reset = () => {
    panels.splice(0);
    dirty.clear();
    project.projectInstanceId = "project-a";
    project.epoch = 1;
    project.available = true;
    chartState.index = [];
    chartState.documents = {};
    fifo = Promise.resolve();

    for (const mock of [
      confirm3,
      saveGraphDraft,
      saveChart,
      clearResourceDocumentState,
      showBlockingIpcError,
      showBlockingMessage,
      releasePane,
      releaseEditorViewport,
      clearDetailFocusForClosedPanel,
      deactivateGraphPanelSession,
      unloadGraphDocument,
      resolveResourceDisplayName,
      setChartState,
      commitRemove,
      runPublicationTransaction,
    ]) {
      mock.mockReset();
    }

    confirm3.mockResolvedValue("discard");
    saveGraphDraft.mockResolvedValue(true);
    saveChart.mockResolvedValue(true);
    unloadGraphDocument.mockResolvedValue(undefined);
    setChartState.mockImplementation(applyChartState);
    resolveResourceDisplayName.mockImplementation((_ref: unknown, fallback: string) => {
      const segments = fallback.split("/");
      const leaf = segments[segments.length - 1] ?? fallback;
      return leaf.replace(/\.yssbi-(event|function|chart)$/, "");
    });
    commitRemove.mockImplementation((tokens: readonly Token[], authorize?: () => boolean) =>
      enqueue(() => {
        if (!isAuthorized(authorize)) return "stale" as const;
        const currentById = new Map(panels.map((panel) => [panel.panelInstanceId, panel]));
        const stale = tokens.some((token) => {
          const current = currentById.get(token.panelInstanceId);
          return (
            !current ||
            current.groupId !== token.groupId ||
            JSON.stringify(current.metadata) !== JSON.stringify(token.metadata)
          );
        });
        if (stale) return "stale" as const;

        const ids = new Set(tokens.map((token) => token.panelInstanceId));
        for (let index = panels.length - 1; index >= 0; index -= 1) {
          if (ids.has(panels[index].panelInstanceId)) panels.splice(index, 1);
        }
        return "committed" as const;
      }),
    );
    runPublicationTransaction.mockImplementation((operation: () => unknown | Promise<unknown>) =>
      enqueue(operation),
    );
  };

  return {
    panels,
    dirty,
    project,
    chartState,
    dirtyKey,
    confirm3,
    saveGraphDraft,
    saveChart,
    clearResourceDocumentState,
    showBlockingIpcError,
    showBlockingMessage,
    releasePane,
    releaseEditorViewport,
    clearDetailFocusForClosedPanel,
    deactivateGraphPanelSession,
    unloadGraphDocument,
    resolveResourceDisplayName,
    setChartState,
    commitRemove,
    runPublicationTransaction,
    reset,
  };
});

vi.mock("i18next", () => ({
  default: {
    t: (key: string, values?: Record<string, string>) => {
      const translations: Record<string, string> = {
        "editor.close.dirtyTitle": "Save changes?",
        "editor.close.dirtyMessage": `“${values?.name ?? ""}” has unsaved changes.`,
        "editor.close.save": "Save",
        "editor.close.discard": "Discard",
        "editor.close.cancel": "Cancel",
        "editor.close.failed": "The panel could not be closed.",
        "notifications.editor.documentSaveFailed": "Document save failed",
      };
      return translations[key] ?? key;
    },
  },
}));

vi.mock("@/modules/workbench/internal/dockview/workbenchRead", () => ({
  workbenchDockviewRead: {
    listPanels: () => mocks.panels,
    listGroupPanels: (groupId: string) => mocks.panels.filter((panel) => panel.groupId === groupId),
  },
}));

vi.mock("@/modules/workbench/internal/dockview/workbenchDockviewInternal", () => ({
  workbenchDockviewRuntime: { control: {} },
  workbenchDockviewInternal: {
    commitRemove: mocks.commitRemove,
    runPublicationTransaction: mocks.runPublicationTransaction,
  },
}));

vi.mock("@/features/core/resource", () => ({
  isResourceDocumentDirty: (ref: { id: string; kind: string }) =>
    mocks.dirty.has(mocks.dirtyKey(ref)),
  clearResourceDocumentState: mocks.clearResourceDocumentState,
}));

vi.mock("@/features/application/graphDraft/saveGraphDraft", () => ({
  saveGraphDraft: mocks.saveGraphDraft,
}));

vi.mock("@/features/core/chart/chartDocumentStore", () => ({
  useChartDocumentStore: {
    getState: () => ({
      ...mocks.chartState,
      saveDocument: mocks.saveChart,
    }),
    setState: mocks.setChartState,
  },
}));

vi.mock("@/features/application/chart/saveChartDocument", () => ({
  saveChartDocument: mocks.saveChart,
}));

vi.mock("@/features/core/projectLifecycle/projectLifecycleAuthority", () => ({
  captureProjectIdentity: () => {
    if (!mocks.project.available) throw new Error("no active project");
    return {
      projectInstanceId: mocks.project.projectInstanceId,
      epoch: mocks.project.epoch,
    };
  },
  isCurrentProjectIdentity: (identity: { projectInstanceId: string; epoch: number }) =>
    mocks.project.available &&
    identity.projectInstanceId === mocks.project.projectInstanceId &&
    identity.epoch === mocks.project.epoch,
}));

vi.mock("./blockingErrorDialog", () => ({
  showBlockingIpcError: mocks.showBlockingIpcError,
  showBlockingMessage: mocks.showBlockingMessage,
}));

vi.mock("@/features/core/ui/UIStore", () => ({
  uiStore: { confirm3: mocks.confirm3 },
}));

vi.mock("@/modules/workbench/internal/dockview/editorPaneStateStore", () => ({
  useEditorPaneStateStore: {
    getState: () => ({ release: mocks.releasePane }),
  },
}));

vi.mock("@/features/core/viewport", () => ({
  editorViewportScope: (groupId: string, graphPath: string) => ({ groupId, graphPath }),
  releaseEditorViewport: mocks.releaseEditorViewport,
}));

vi.mock("@/features/application/editor/clearDetailFocusForClosedPanel", () => ({
  clearDetailFocusForClosedPanel: mocks.clearDetailFocusForClosedPanel,
}));

vi.mock("./graphPanelSession", () => ({
  deactivateGraphPanelSession: mocks.deactivateGraphPanelSession,
}));

vi.mock("./graphDocumentUnload", () => ({
  unloadGraphDocument: mocks.unloadGraphDocument,
}));

vi.mock("./resolveResourceDisplayName", () => ({
  resolveResourceDisplayName: mocks.resolveResourceDisplayName,
}));

import {
  requestCloseWorkbenchGroup,
  requestCloseWorkbenchPanel,
  requestCloseWorkbenchPanels,
} from "./workbenchPanelClose";

function editorPanel(
  panelInstanceId: string,
  resourceRef: string,
  resourceKind: EditorResourceKind = "event",
  groupId = "group-a",
): WorkbenchPanelInfo {
  return {
    panelInstanceId,
    groupId,
    component: "EditorResource",
    title: resourceRef,
    metadata: { role: "editor", resourceRef, resourceKind },
    active: false,
    location: { type: "grid" },
  };
}

function viewPanel(
  panelInstanceId: string,
  viewId: WorkbenchViewId = "logs",
  groupId = "group-a",
): WorkbenchPanelInfo {
  const components = {
    project: "Project",
    nodes: "Nodes",
    data: "Data",
    commands: "Commands",
    details: "Details",
    assistant: "Assistant",
    inspect: "Inspect",
    logs: "Logs",
    output: "Output",
    diagnostics: "Diagnostics",
  } as const;
  return {
    panelInstanceId,
    groupId,
    component: components[viewId],
    title: viewId,
    metadata: { role: "view", viewId },
    active: false,
    location: { type: "grid" },
  };
}

function resultPanel(panelInstanceId: string, groupId = "group-a"): WorkbenchPanelInfo {
  return {
    panelInstanceId,
    groupId,
    component: "Result",
    title: "Result",
    metadata: {
      role: "result",
      resultKey: "output:main",
      resultId: "result-a",
      title: "Result",
      presentation: { kind: "inspector" },
      source: null,
    },
    active: false,
    location: { type: "grid" },
  };
}

function seedPanels(panels: readonly WorkbenchPanelInfo[]): void {
  mocks.panels.push(...panels);
}

function markDirty(resourceRef: string, resourceKind: EditorResourceKind): void {
  mocks.dirty.add(mocks.dirtyKey({ id: resourceRef, kind: resourceKind }));
}

beforeEach(() => {
  mocks.reset();
});

describe("workbench panel close coordinator", () => {
  it("resolves a physical group before entering the batch close workflow", async () => {
    seedPanels([
      viewPanel("logs-a", "logs", "group-a"),
      viewPanel("output-a", "output", "group-a"),
      viewPanel("diagnostics-b", "diagnostics", "group-b"),
    ]);

    await expect(requestCloseWorkbenchGroup("group-a")).resolves.toBe(true);

    expect(mocks.panels.map((panel) => panel.panelInstanceId)).toEqual(["diagnostics-b"]);
  });

  it("removes nothing when a dirty editor cancels a mixed Close Group", async () => {
    const graphPath = "events/Main.yssbi-event";
    seedPanels([editorPanel("editor-a", graphPath), viewPanel("logs-a"), resultPanel("result-a")]);
    markDirty(graphPath, "event");
    mocks.confirm3.mockResolvedValueOnce("cancel");

    await expect(requestCloseWorkbenchPanels(["editor-a", "logs-a", "result-a"])).resolves.toBe(
      false,
    );

    expect(mocks.confirm3).toHaveBeenCalledOnce();
    expect(mocks.confirm3).toHaveBeenCalledWith({
      title: "Save changes?",
      message: "“Main” has unsaved changes.",
      confirmText: "Save",
      discardText: "Discard",
      cancelText: "Cancel",
      type: "info",
    });
    expect(mocks.commitRemove).not.toHaveBeenCalled();
    expect(mocks.panels.map((panel) => panel.panelInstanceId)).toEqual([
      "editor-a",
      "logs-a",
      "result-a",
    ]);
    expect(mocks.releasePane).not.toHaveBeenCalled();
  });

  it("prompts once for duplicate editors and finalizes only committed resources", async () => {
    const graphPath = "events/Main.yssbi-event";
    const chartPath = "charts/Summary.yssbi-chart";
    seedPanels([
      editorPanel("editor-a", graphPath, "event", "group-a"),
      editorPanel("editor-b", graphPath, "event", "group-b"),
      editorPanel("chart-a", chartPath, "chart", "group-b"),
      viewPanel("logs-a", "logs", "group-b"),
      resultPanel("result-a", "group-b"),
    ]);
    markDirty(graphPath, "event");
    mocks.confirm3.mockResolvedValueOnce("discard");

    await expect(
      requestCloseWorkbenchPanels([
        "editor-a",
        "editor-b",
        "chart-a",
        "logs-a",
        "result-a",
        "editor-a",
      ]),
    ).resolves.toBe(true);

    expect(mocks.confirm3).toHaveBeenCalledOnce();
    expect(mocks.commitRemove).toHaveBeenCalledOnce();
    const tokens = mocks.commitRemove.mock.calls[0][0] as readonly WorkbenchPanelCommitToken[];
    expect(tokens.map(({ panelInstanceId, groupId }) => ({ panelInstanceId, groupId }))).toEqual([
      { panelInstanceId: "editor-a", groupId: "group-a" },
      { panelInstanceId: "editor-b", groupId: "group-b" },
      { panelInstanceId: "chart-a", groupId: "group-b" },
      { panelInstanceId: "logs-a", groupId: "group-b" },
      { panelInstanceId: "result-a", groupId: "group-b" },
    ]);
    expect(mocks.releasePane.mock.calls.map(([panelId]) => panelId)).toEqual([
      "editor-a",
      "editor-b",
      "chart-a",
    ]);
    expect(mocks.releaseEditorViewport).toHaveBeenCalledTimes(2);
    expect(mocks.releaseEditorViewport).toHaveBeenCalledWith({
      groupId: "group-a",
      graphPath,
    });
    expect(mocks.releaseEditorViewport).toHaveBeenCalledWith({
      groupId: "group-b",
      graphPath,
    });
    expect(mocks.clearDetailFocusForClosedPanel).toHaveBeenCalledWith(graphPath);
    expect(mocks.clearDetailFocusForClosedPanel).toHaveBeenCalledWith(chartPath);
    expect(mocks.clearResourceDocumentState).toHaveBeenCalledTimes(2);
    expect(mocks.clearResourceDocumentState).toHaveBeenCalledWith({
      id: graphPath,
      kind: "event",
    });
    expect(mocks.clearResourceDocumentState).toHaveBeenCalledWith({
      id: chartPath,
      kind: "chart",
    });
    expect(mocks.unloadGraphDocument).toHaveBeenCalledOnce();
    expect(mocks.unloadGraphDocument).toHaveBeenCalledWith(graphPath);
    expect(mocks.unloadGraphDocument).not.toHaveBeenCalledWith(chartPath);
  });

  it("does not preflight or release shared state when another editor for the resource remains", async () => {
    const graphPath = "events/Main.yssbi-event";
    seedPanels([
      editorPanel("editor-a", graphPath, "event", "group-a"),
      editorPanel("editor-b", graphPath, "event", "group-a"),
    ]);
    markDirty(graphPath, "event");

    await expect(requestCloseWorkbenchPanel("editor-a")).resolves.toBe(true);

    expect(mocks.confirm3).not.toHaveBeenCalled();
    expect(mocks.releasePane).toHaveBeenCalledWith("editor-a");
    expect(mocks.releaseEditorViewport).not.toHaveBeenCalled();
    expect(mocks.clearDetailFocusForClosedPanel).not.toHaveBeenCalled();
    expect(mocks.clearResourceDocumentState).not.toHaveBeenCalled();
    expect(mocks.unloadGraphDocument).not.toHaveBeenCalled();
    expect(mocks.panels.map((panel) => panel.panelInstanceId)).toEqual(["editor-b"]);
  });

  it("releases a closed graph scope while another group keeps the document open", async () => {
    const graphPath = "events/Main.yssbi-event";
    seedPanels([
      editorPanel("editor-a", graphPath, "event", "group-a"),
      editorPanel("editor-b", graphPath, "event", "group-b"),
    ]);
    markDirty(graphPath, "event");

    await expect(requestCloseWorkbenchPanel("editor-a")).resolves.toBe(true);

    expect(mocks.confirm3).not.toHaveBeenCalled();
    expect(mocks.releasePane).toHaveBeenCalledWith("editor-a");
    expect(mocks.releaseEditorViewport).toHaveBeenCalledOnce();
    expect(mocks.releaseEditorViewport).toHaveBeenCalledWith({
      groupId: "group-a",
      graphPath,
    });
    expect(mocks.deactivateGraphPanelSession).toHaveBeenCalledOnce();
    expect(mocks.deactivateGraphPanelSession).toHaveBeenCalledWith("group-a", graphPath);
    expect(mocks.clearDetailFocusForClosedPanel).not.toHaveBeenCalled();
    expect(mocks.clearResourceDocumentState).not.toHaveBeenCalled();
    expect(mocks.unloadGraphDocument).not.toHaveBeenCalled();
    expect(mocks.panels.map((panel) => panel.panelInstanceId)).toEqual(["editor-b"]);
  });

  it("serializes concurrent duplicate closes so only the true-last request prompts", async () => {
    const graphPath = "events/Main.yssbi-event";
    seedPanels([
      editorPanel("editor-a", graphPath, "event", "group-a"),
      editorPanel("editor-b", graphPath, "event", "group-b"),
    ]);
    markDirty(graphPath, "event");
    mocks.confirm3.mockResolvedValueOnce("cancel");

    const outcomes = await Promise.all([
      requestCloseWorkbenchPanel("editor-a"),
      requestCloseWorkbenchPanel("editor-b"),
    ]);

    expect(outcomes).toEqual([true, false]);
    expect(mocks.confirm3).toHaveBeenCalledOnce();
    expect(mocks.panels.map((panel) => panel.panelInstanceId)).toEqual(["editor-b"]);
    expect(mocks.clearResourceDocumentState).not.toHaveBeenCalled();
    expect(mocks.unloadGraphDocument).not.toHaveBeenCalled();
  });

  it("rejects project replacement queued ahead of committed removal", async () => {
    const graphPath = "events/Main.yssbi-event";
    seedPanels([editorPanel("editor-a", graphPath)]);
    let releaseReplacement!: () => void;
    let markReplacementStarted!: () => void;
    const replacementGate = new Promise<void>((resolve) => {
      releaseReplacement = resolve;
    });
    const replacementStarted = new Promise<void>((resolve) => {
      markReplacementStarted = resolve;
    });
    const queuedReplacement = mocks.runPublicationTransaction(async () => {
      markReplacementStarted();
      await replacementGate;
      mocks.project.epoch += 1;
    });
    await replacementStarted;

    const closing = requestCloseWorkbenchPanel("editor-a");
    await Promise.resolve();
    releaseReplacement();
    await queuedReplacement;

    await expect(closing).resolves.toBe(false);
    expect(mocks.commitRemove).toHaveBeenCalledOnce();
    expect(mocks.commitRemove.mock.calls[0][1]).toEqual(expect.any(Function));
    expect(mocks.panels.map((panel) => panel.panelInstanceId)).toEqual(["editor-a"]);
    expect(mocks.releasePane).not.toHaveBeenCalled();
  });

  it("stops before commit when the project changes during dirty confirmation", async () => {
    const graphPath = "events/Main.yssbi-event";
    seedPanels([editorPanel("editor-a", graphPath)]);
    markDirty(graphPath, "event");
    let resolveDecision!: (decision: "discard") => void;
    let markPrompted!: () => void;
    const prompted = new Promise<void>((resolve) => {
      markPrompted = resolve;
    });
    mocks.confirm3.mockImplementationOnce(() => {
      markPrompted();
      return new Promise<"discard">((resolve) => {
        resolveDecision = resolve;
      });
    });

    const closing = requestCloseWorkbenchPanel("editor-a");
    await prompted;
    mocks.project.epoch += 1;
    resolveDecision("discard");

    await expect(closing).resolves.toBe(false);
    expect(mocks.commitRemove).not.toHaveBeenCalled();
    expect(mocks.panels).toHaveLength(1);
  });

  it("suppresses stale-project feedback when Graph save rejects", async () => {
    const graphPath = "events/Main.yssbi-event";
    seedPanels([editorPanel("editor-a", graphPath)]);
    markDirty(graphPath, "event");
    mocks.confirm3.mockResolvedValueOnce("confirm");
    mocks.saveGraphDraft.mockImplementationOnce(async () => {
      mocks.project.epoch += 1;
      throw new Error("stale project internals");
    });

    await expect(requestCloseWorkbenchPanel("editor-a")).resolves.toBe(false);

    expect(mocks.showBlockingIpcError).not.toHaveBeenCalled();
    expect(mocks.showBlockingMessage).not.toHaveBeenCalled();
    expect(mocks.commitRemove).not.toHaveBeenCalled();
    expect(mocks.panels).toHaveLength(1);
  });

  it("allows projectless tool closes but rejects project-scoped panels", async () => {
    mocks.project.available = false;
    seedPanels([viewPanel("logs-a", "logs"), viewPanel("output-a", "output")]);

    await expect(requestCloseWorkbenchPanels(["logs-a", "output-a"])).resolves.toBe(true);

    seedPanels([
      editorPanel("editor-a", "events/Main.yssbi-event"),
      resultPanel("result-a"),
      viewPanel("details-a", "details"),
      viewPanel("inspect-a", "inspect"),
    ]);
    await expect(
      requestCloseWorkbenchPanels(["editor-a", "result-a", "details-a", "inspect-a"]),
    ).resolves.toBe(false);
    expect(mocks.panels.map((panel) => panel.panelInstanceId)).toEqual([
      "editor-a",
      "result-a",
      "details-a",
      "inspect-a",
    ]);
  });

  it("finalizes only absent panels after a partial native close failure", async () => {
    const firstPath = "events/First.yssbi-event";
    const secondPath = "functions/Second.yssbi-function";
    seedPanels([
      editorPanel("editor-a", firstPath, "event", "group-a"),
      editorPanel("editor-b", secondPath, "function", "group-b"),
    ]);
    mocks.commitRemove.mockImplementationOnce(async (tokens, authorize) => {
      if (authorize && !authorize()) return "stale" as const;
      const firstId = tokens[0].panelInstanceId;
      const index = mocks.panels.findIndex((panel) => panel.panelInstanceId === firstId);
      if (index >= 0) mocks.panels.splice(index, 1);
      throw Object.assign(new Error("private native close detail"), {
        code: "layout_restore_failed",
      });
    });

    await expect(requestCloseWorkbenchPanels(["editor-a", "editor-b"])).resolves.toBe(false);

    expect(mocks.panels.map((panel) => panel.panelInstanceId)).toEqual(["editor-b"]);
    expect(mocks.releasePane).toHaveBeenCalledOnce();
    expect(mocks.releasePane).toHaveBeenCalledWith("editor-a");
    expect(mocks.releaseEditorViewport).toHaveBeenCalledWith({
      groupId: "group-a",
      graphPath: firstPath,
    });
    expect(mocks.deactivateGraphPanelSession).toHaveBeenCalledWith("group-a", firstPath);
    expect(mocks.clearDetailFocusForClosedPanel).toHaveBeenCalledWith(firstPath);
    expect(mocks.clearDetailFocusForClosedPanel).not.toHaveBeenCalledWith(secondPath);
    expect(mocks.clearResourceDocumentState).toHaveBeenCalledWith({
      id: firstPath,
      kind: "event",
    });
    expect(mocks.unloadGraphDocument).toHaveBeenCalledWith(firstPath);
    expect(mocks.unloadGraphDocument).not.toHaveBeenCalledWith(secondPath);
    expect(mocks.showBlockingMessage).toHaveBeenCalledOnce();
    expect(mocks.showBlockingMessage).toHaveBeenCalledWith("The panel could not be closed.");
    expect(JSON.stringify(mocks.showBlockingMessage.mock.calls)).not.toContain(
      "private native close detail",
    );
    expect(mocks.showBlockingIpcError).not.toHaveBeenCalled();
  });

  it("keeps an earlier save when a later dirty document cancels the batch", async () => {
    const firstPath = "events/First.yssbi-event";
    const secondPath = "functions/Second.yssbi-function";
    seedPanels([
      editorPanel("editor-a", firstPath, "event", "group-a"),
      editorPanel("editor-b", secondPath, "function", "group-b"),
    ]);
    markDirty(firstPath, "event");
    markDirty(secondPath, "function");
    mocks.confirm3.mockResolvedValueOnce("confirm").mockResolvedValueOnce("cancel");

    await expect(requestCloseWorkbenchPanels(["editor-a", "editor-b"])).resolves.toBe(false);

    expect(mocks.saveGraphDraft).toHaveBeenCalledOnce();
    expect(mocks.saveGraphDraft).toHaveBeenCalledWith(firstPath, "event");
    expect(mocks.commitRemove).not.toHaveBeenCalled();
    expect(mocks.panels).toHaveLength(2);
  });

  it("removes nothing when the existing chart save lifecycle does not commit", async () => {
    const chartPath = "charts/Summary.yssbi-chart";
    seedPanels([editorPanel("chart-a", chartPath, "chart")]);
    markDirty(chartPath, "chart");
    mocks.confirm3.mockResolvedValueOnce("confirm");
    mocks.saveChart.mockResolvedValueOnce(false);

    await expect(requestCloseWorkbenchPanel("chart-a")).resolves.toBe(false);

    expect(mocks.saveChart).toHaveBeenCalledWith(chartPath);
    expect(mocks.saveGraphDraft).not.toHaveBeenCalled();
    expect(mocks.showBlockingMessage).toHaveBeenCalledOnce();
    expect(mocks.commitRemove).not.toHaveBeenCalled();
    expect(mocks.releasePane).not.toHaveBeenCalled();
    expect(mocks.unloadGraphDocument).not.toHaveBeenCalled();
    expect(mocks.panels).toHaveLength(1);
  });

  it("evicts the last discarded chart document while preserving its index entry", async () => {
    const chartPath = "charts/Summary.yssbi-chart";
    const indexEntry = { chartPath, name: "Summary" };
    const document = { revision: 7, chartType: "scatter" };
    mocks.chartState.index = [indexEntry];
    mocks.chartState.documents = { [chartPath]: document };
    seedPanels([editorPanel("chart-a", chartPath, "chart")]);
    markDirty(chartPath, "chart");
    mocks.confirm3.mockResolvedValueOnce("discard");

    await expect(requestCloseWorkbenchPanel("chart-a")).resolves.toBe(true);

    expect(mocks.chartState.documents).not.toHaveProperty(chartPath);
    expect(mocks.chartState.index).toEqual([indexEntry]);
    expect(mocks.clearResourceDocumentState).toHaveBeenCalledWith({
      id: chartPath,
      kind: "chart",
    });
    expect(mocks.clearDetailFocusForClosedPanel).toHaveBeenCalledWith(chartPath);
    expect(mocks.unloadGraphDocument).not.toHaveBeenCalled();
  });

  it.each(["missing", "invalid"] as const)(
    "rejects a %s target before prompting or committing any panel",
    async (caseName) => {
      const graphPath = "events/Main.yssbi-event";
      seedPanels([editorPanel("editor-a", graphPath)]);
      markDirty(graphPath, "event");
      if (caseName === "invalid") {
        mocks.panels.push({
          ...viewPanel("invalid-a"),
          metadata: { role: "unknown" } as unknown as WorkbenchPanelMetadata,
        });
      }
      const requested =
        caseName === "missing" ? ["editor-a", "missing-a"] : ["editor-a", "invalid-a"];

      await expect(requestCloseWorkbenchPanels(requested)).resolves.toBe(false);

      expect(mocks.confirm3).not.toHaveBeenCalled();
      expect(mocks.commitRemove).not.toHaveBeenCalled();
      expect(mocks.panels.map((panel) => panel.panelInstanceId)).toContain("editor-a");
    },
  );

  it("rejects a queued metadata/group race without removing or finalizing any panel", async () => {
    const graphPath = "events/Main.yssbi-event";
    seedPanels([editorPanel("editor-a", graphPath, "event", "group-a")]);
    let releaseMutation!: () => void;
    let markMutationStarted!: () => void;
    const mutationGate = new Promise<void>((resolve) => {
      releaseMutation = resolve;
    });
    const mutationStarted = new Promise<void>((resolve) => {
      markMutationStarted = resolve;
    });
    const queuedMutation = mocks.runPublicationTransaction(async () => {
      markMutationStarted();
      await mutationGate;
      const panel = mocks.panels[0];
      panel.groupId = "group-b";
      panel.metadata = { ...(panel.metadata as object), pinned: true };
    });
    await mutationStarted;

    const closing = requestCloseWorkbenchPanel("editor-a");
    await Promise.resolve();
    releaseMutation();
    await queuedMutation;

    await expect(closing).resolves.toBe(false);
    expect(mocks.commitRemove).toHaveBeenCalledWith(
      [
        expect.objectContaining({
          panelInstanceId: "editor-a",
          groupId: "group-a",
          metadata: { role: "editor", resourceRef: graphPath, resourceKind: "event" },
        }),
      ],
      expect.any(Function),
    );
    expect(mocks.panels).toHaveLength(1);
    expect(mocks.panels[0]).toMatchObject({
      panelInstanceId: "editor-a",
      groupId: "group-b",
      metadata: { pinned: true },
    });
    expect(mocks.releasePane).not.toHaveBeenCalled();
    expect(mocks.releaseEditorViewport).not.toHaveBeenCalled();
    expect(mocks.unloadGraphDocument).not.toHaveBeenCalled();
  });
});
