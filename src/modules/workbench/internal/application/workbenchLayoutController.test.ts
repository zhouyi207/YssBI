import type { DockviewApi, IDockviewPanel, SerializedDockview } from "dockview-react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  createDefaultLogsDockviewLayout,
  LOGS_DOCKVIEW_COMPONENT_ID,
} from "../dockview/logsDockviewLayout";
import { createLogsDockviewRuntime } from "../dockview/logsRuntime";
import type {
  WorkbenchDockviewInternal,
  WorkbenchLayoutTransaction,
} from "../dockview/workbenchDockviewInternal";
import type { WorkbenchDockviewControl } from "../dockview/workbenchControl";
import type { WorkbenchDockviewRead, WorkbenchPanelInfo } from "../dockview/workbenchRead";
import type { WorkbenchComponentId, WorkbenchViewId } from "../dockview/workbenchPanelModel";
import { createWorkbenchLayoutController } from "./workbenchLayoutController";

type Listener = () => void;
type MutableGroup = {
  views: string[];
  activeView?: string;
};
type Deferred<T> = {
  readonly promise: Promise<T>;
  readonly resolve: (value: T) => void;
};

function createDeferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((next) => {
    resolve = next;
  });
  return { promise, resolve };
}

function assertRestorableLayout(layout: SerializedDockview): void {
  const root = layout.grid.root;
  if (root.type !== "branch" || !Array.isArray(root.data)) {
    throw new Error("Dockview layouts require a top-level branch");
  }
}

function getOnlyGridGroup(layout: SerializedDockview): MutableGroup {
  const root = layout.grid.root;
  if (root.type !== "branch" || !Array.isArray(root.data) || root.data.length !== 1) {
    throw new Error("fixture must use a top-level branch with one group");
  }
  const child = root.data[0];
  if (child.type !== "leaf") throw new Error("fixture branch child must be a group leaf");
  return child.data as MutableGroup;
}

function rootLayout(
  panelId = "logs-default",
  viewId: WorkbenchViewId = "logs",
): SerializedDockview {
  const components: Record<WorkbenchViewId, string> = {
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
  };
  const activityPanels = {
    "project-stable": {
      id: "project-stable",
      contentComponent: "Project",
      title: "Project",
      params: { metadata: { role: "view", viewId: "project" } },
    },
    "nodes-stable": {
      id: "nodes-stable",
      contentComponent: "Nodes",
      title: "Nodes",
      params: { metadata: { role: "view", viewId: "nodes" } },
    },
    "data-stable": {
      id: "data-stable",
      contentComponent: "Data",
      title: "Data",
      params: { metadata: { role: "view", viewId: "data" } },
    },
    "commands-stable": {
      id: "commands-stable",
      contentComponent: "Commands",
      title: "Commands",
      params: { metadata: { role: "view", viewId: "commands" } },
    },
  };
  return {
    grid: {
      root: {
        type: "branch",
        data: [
          {
            type: "leaf",
            data: { id: "grid-main", views: [panelId], activeView: panelId },
          },
        ],
      },
      height: 800,
      width: 1200,
      orientation: "HORIZONTAL",
    },
    panels: {
      [panelId]: {
        id: panelId,
        contentComponent: components[viewId],
        title: components[viewId],
        params: { metadata: { role: "view", viewId } },
      },
      ...activityPanels,
    },
    activeGroup: "grid-main",
    floatingGroups: [],
    popoutGroups: [],
    edgeGroups: {
      left: {
        size: 292,
        visible: true,
        collapsed: false,
        group: {
          id: "workbench-edge-left",
          views: ["project-stable", "nodes-stable", "data-stable", "commands-stable"],
          activeView: "project-stable",
          headerPosition: "left",
        },
      },
    },
  } as unknown as SerializedDockview;
}

function projectRootLayout(): SerializedDockview {
  return {
    grid: {
      root: {
        type: "branch",
        data: [
          {
            type: "leaf",
            data: {
              id: "editor-group",
              views: ["editor-old-project"],
              activeView: "editor-old-project",
            },
          },
        ],
      },
      height: 800,
      width: 1200,
      orientation: "HORIZONTAL",
      maximizedNode: { location: [0] },
    },
    panels: {
      "editor-old-project": {
        id: "editor-old-project",
        contentComponent: "EditorResource",
        title: "Shared graph",
        params: {
          metadata: {
            role: "editor",
            resourceRef: "events/Shared.yssbi-event",
            resourceKind: "event",
          },
        },
      },
      "project-stable": {
        id: "project-stable",
        contentComponent: "Project",
        title: "Project",
        params: { metadata: { role: "view", viewId: "project" } },
      },
      "nodes-stable": {
        id: "nodes-stable",
        contentComponent: "Nodes",
        title: "Nodes",
        params: { metadata: { role: "view", viewId: "nodes" } },
      },
      "data-stable": {
        id: "data-stable",
        contentComponent: "Data",
        title: "Data",
        params: { metadata: { role: "view", viewId: "data" } },
      },
      "commands-stable": {
        id: "commands-stable",
        contentComponent: "Commands",
        title: "Commands",
        params: { metadata: { role: "view", viewId: "commands" } },
      },
      "logs-stable": {
        id: "logs-stable",
        contentComponent: "Logs",
        title: "Logs",
        params: { metadata: { role: "view", viewId: "logs" } },
      },
      "output-stable": {
        id: "output-stable",
        contentComponent: "Output",
        title: "Output",
        params: { metadata: { role: "view", viewId: "output" } },
      },
    },
    activeGroup: "editor-group",
    floatingGroups: [],
    popoutGroups: [],
    edgeGroups: {
      left: {
        size: 292,
        visible: true,
        collapsed: false,
        group: {
          id: "workbench-edge-left",
          views: ["project-stable", "nodes-stable", "data-stable", "commands-stable"],
          activeView: "project-stable",
          headerPosition: "left",
        },
      },
      bottom: {
        size: 200,
        visible: true,
        collapsed: true,
        group: {
          id: "bottom-tools",
          views: ["logs-stable", "output-stable"],
          activeView: "output-stable",
        },
      },
    },
  } as unknown as SerializedDockview;
}

function emptyRootLayout(): SerializedDockview {
  return {
    grid: {
      root: { type: "branch", data: [] },
      height: 800,
      width: 1200,
      orientation: "HORIZONTAL",
    },
    panels: {},
    floatingGroups: [],
    popoutGroups: [],
  } as unknown as SerializedDockview;
}

function invalidRootLayout(): SerializedDockview {
  const layout = rootLayout();
  layout.panels["logs-default"].contentComponent = "Output";
  return layout;
}

function logsLayout(activeDomain: string): SerializedDockview {
  const layout = createDefaultLogsDockviewLayout();
  getOnlyGridGroup(layout).activeView = `logs-domain:${activeDomain}`;
  return layout;
}

function invalidLogsLayout(): SerializedDockview {
  const layout = createDefaultLogsDockviewLayout();
  const group = getOnlyGridGroup(layout);
  const duplicateId = "logs-domain:all-copy";
  layout.panels[duplicateId] = {
    id: duplicateId,
    contentComponent: LOGS_DOCKVIEW_COMPONENT_ID,
    title: "All copy",
    params: { domain: "all" },
  };
  group.views.push(duplicateId);
  return layout;
}

function storedPayload(
  root: unknown = rootLayout(),
  logs: unknown = createDefaultLogsDockviewLayout(),
): string {
  return JSON.stringify({
    root,
    nested: { logs },
  });
}

function panelInfo(viewId: WorkbenchViewId): WorkbenchPanelInfo {
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
  } as const satisfies Record<WorkbenchViewId, WorkbenchComponentId>;
  const component = components[viewId];
  return {
    panelInstanceId: `view:${viewId}`,
    groupId: "grid-main",
    component,
    metadata: { role: "view", viewId },
    active: false,
    location: { type: "grid" },
  };
}

function createFakeRoot(initialLayout: SerializedDockview | undefined, order: string[]) {
  const startingLayout = initialLayout ?? emptyRootLayout();
  assertRestorableLayout(startingLayout);
  let layout = structuredClone(startingLayout);
  const panels: IDockviewPanel[] = [];
  const syncPanels = (): void => {
    panels.splice(
      0,
      panels.length,
      ...Object.keys(layout.panels).map((id) => ({ id }) as IDockviewPanel),
    );
  };
  syncPanels();

  const fromJSON = vi.fn((next: SerializedDockview) => {
    assertRestorableLayout(next);
    order.push("root.fromJSON");
    layout = structuredClone(next);
    syncPanels();
  });
  const toJSON = vi.fn(() => {
    order.push("root.toJSON");
    return structuredClone(layout);
  });
  const api = {
    get panels() {
      return panels;
    },
    fromJSON,
    toJSON,
  } as unknown as DockviewApi;

  return { api, fromJSON, toJSON };
}

function createFakePort(order: string[], outputMoveGate?: Deferred<void>) {
  type FakeEditorOpenRequest = {
    readonly resourceRef: string;
    readonly resourceKind: "event";
    readonly title: string;
    readonly pinned: boolean;
  };
  type PendingEditorOpen = {
    readonly request: FakeEditorOpenRequest;
    readonly resolve: (panel: WorkbenchPanelInfo) => void;
  };

  const listeners = new Set<Listener>();
  const pendingEditorOpens: PendingEditorOpen[] = [];
  const layoutOperationHydrationStates: boolean[] = [];
  let hydrated = false;
  let hydrationEpoch = 0;
  let idle: Promise<void> = Promise.resolve();
  let serialized = rootLayout("serialized-output", "output");

  const subscribe = vi.fn((listener: Listener) => {
    order.push("port.subscribe");
    listeners.add(listener);
    return () => {
      order.push("port.unsubscribe");
      listeners.delete(listener);
    };
  });
  const ensureCentralGroup = vi.fn(async () => {
    order.push("port.ensureCentralGroup");
    return "grid-main";
  });
  const ensureView = vi.fn(async ({ viewId }: { viewId: WorkbenchViewId }) => {
    order.push(`port.ensureView:${viewId}`);
    if (
      ![
        "project",
        "nodes",
        "data",
        "commands",
        "details",
        "assistant",
        "logs",
        "output",
        "diagnostics",
      ].includes(viewId)
    ) {
      throw new Error(`unexpected default view ${viewId}`);
    }
    return panelInfo(viewId);
  });
  const configureEdge = vi.fn(
    async (request: {
      position: "left" | "bottom" | "top" | "right";
      size: number;
      collapsed: boolean;
      headerPosition?: "top" | "bottom" | "left" | "right";
    }) => {
      order.push(`port.configureEdge:${request.position}`);
      return {
        position: request.position,
        exists: true as const,
        groupId: `edge-${request.position}`,
        visible: true,
        collapsed: request.collapsed,
        size: request.size,
      };
    },
  );
  const move = vi.fn(
    async (request: { panelInstanceId: string; groupId: string; index?: number }) => {
      order.push(`port.move:${request.panelInstanceId}`);
      if (request.panelInstanceId === "view:output" && outputMoveGate) {
        await outputMoveGate.promise;
      }
      return true;
    },
  );
  const serialize = vi.fn(async () => {
    order.push("port.serialize");
    return structuredClone(serialized);
  });
  const executeEditorOpen = (request: FakeEditorOpenRequest): WorkbenchPanelInfo => {
    order.push(`port.openEditor:${request.resourceRef}`);
    return {
      panelInstanceId: `editor:${request.resourceRef}`,
      groupId: "grid-main",
      component: "EditorResource",
      title: request.title,
      metadata: {
        role: "editor",
        resourceRef: request.resourceRef,
        resourceKind: request.resourceKind,
        pinned: request.pinned,
      },
      active: true,
      location: { type: "grid" },
    };
  };
  const openEditor = vi.fn((request: FakeEditorOpenRequest) => {
    if (hydrated) return Promise.resolve(executeEditorOpen(request));
    return new Promise<WorkbenchPanelInfo>((resolve) => {
      pendingEditorOpens.push({ request, resolve });
    });
  });

  const recordLayoutOperation = (name: string): void => {
    layoutOperationHydrationStates.push(hydrated);
    order.push(name);
  };
  const layoutEnsureCentralGroup = vi.fn(() => {
    recordLayoutOperation("layout.ensureCentralGroup");
    return "grid-main";
  });
  const layoutEnsureView = vi.fn(({ viewId }: { viewId: WorkbenchViewId }) => {
    recordLayoutOperation(`layout.ensureView:${viewId}`);
    if (
      ![
        "project",
        "nodes",
        "data",
        "commands",
        "details",
        "assistant",
        "logs",
        "output",
        "diagnostics",
      ].includes(viewId)
    ) {
      throw new Error(`unexpected default view ${viewId}`);
    }
    return panelInfo(viewId);
  });
  const layoutConfigureEdge = vi.fn(
    (request: {
      position: "left" | "bottom" | "top" | "right";
      size: number;
      collapsed: boolean;
      headerPosition?: "top" | "bottom" | "left" | "right";
    }) => {
      recordLayoutOperation(`layout.configureEdge:${request.position}`);
      return {
        position: request.position,
        exists: true as const,
        groupId: `edge-${request.position}`,
        visible: true,
        collapsed: request.collapsed,
        size: request.size,
      };
    },
  );
  const layoutMove = vi.fn(
    (request: { panelInstanceId: string; groupId: string; index?: number; activate?: boolean }) => {
      recordLayoutOperation(`layout.move:${request.panelInstanceId}`);
      return true;
    },
  );
  const layoutActivate = vi.fn((panelInstanceId: string) => {
    order.push(`layout.activate:${panelInstanceId}`);
    return true;
  });
  const layoutTransaction = {
    serialize: () => structuredClone(serialized),
    getPanel: vi.fn(() => undefined),
    getActivePanel: vi.fn(() => undefined),
    ensureCentralGroup: layoutEnsureCentralGroup,
    ensureView: layoutEnsureView,
    configureEdge: layoutConfigureEdge,
    move: layoutMove,
    activate: layoutActivate,
  } as unknown as WorkbenchLayoutTransaction;
  const installHydrationLayout = vi.fn(
    (epoch: number, operation: (transaction: WorkbenchLayoutTransaction) => unknown) => {
      order.push("internal.installHydrationLayout:start");
      if (hydrated || epoch !== hydrationEpoch) {
        throw new Error("invalid fake hydration layout transaction");
      }
      const result = operation(layoutTransaction);
      order.push("internal.installHydrationLayout:applied");
      return result;
    },
  );
  const runLayoutTransaction = vi.fn(
    async (operation: (transaction: WorkbenchLayoutTransaction) => unknown) => {
      order.push("internal.runLayoutTransaction:start");
      const result = operation(layoutTransaction);
      order.push("internal.runLayoutTransaction:applied");
      return result;
    },
  );

  const completeHydration = vi.fn((epoch?: number) => {
    order.push("internal.completeHydration");
    if (epoch !== undefined && epoch !== hydrationEpoch) return;
    hydrated = true;
    pendingEditorOpens.splice(0).forEach(({ request, resolve }) => {
      resolve(executeEditorOpen(request));
    });
  });

  const port = {
    get isReady() {
      return true;
    },
    get isHydrated() {
      return hydrated;
    },
    subscribe,
    openEditor,
    ensureCentralGroup,
    ensureView,
    configureEdge,
    move,
    serialize,
  } as unknown as WorkbenchDockviewRead & WorkbenchDockviewControl;

  const internal = {
    bind: vi.fn(() => {
      order.push("internal.bind");
    }),
    unbind: vi.fn(() => {
      order.push("internal.unbind");
    }),
    beginHydration: vi.fn(() => {
      hydrated = false;
      hydrationEpoch += 1;
      order.push("internal.beginHydration");
      return hydrationEpoch;
    }),
    completeHydration,
    invalidateHydration: vi.fn(),
    invalidatePendingOperations: vi.fn(() => {
      order.push("internal.invalidatePendingOperations");
    }),
    installHydrationLayout,
    runLayoutTransaction,
    whenIdle: vi.fn(() => {
      order.push("internal.whenIdle");
      return idle;
    }),
  } as unknown as WorkbenchDockviewInternal;

  return {
    port,
    read: port,
    control: port,
    internal,
    subscribe,
    openEditor,
    ensureCentralGroup,
    ensureView,
    configureEdge,
    move,
    serialize,
    installHydrationLayout,
    runLayoutTransaction,
    completeHydration,
    layoutEnsureCentralGroup,
    layoutEnsureView,
    layoutConfigureEdge,
    layoutMove,
    layoutOperationHydrationStates,
    emitLayoutChange(): void {
      [...listeners].forEach((listener) => listener());
    },
    setIdle(next: Promise<void>): void {
      idle = next;
    },
    setSerialized(next: SerializedDockview): void {
      serialized = structuredClone(next);
    },
  };
}

function createStorage(order: string[], raw: string | null) {
  const values = new Map<string, string>();
  if (raw !== null) values.set("yssbi-workbench-layout:main", raw);
  const getItem = vi.fn((key: string) => values.get(key) ?? null);
  const setItem = vi.fn((key: string, value: string) => {
    order.push("storage.setItem");
    values.set(key, value);
  });
  return { values, getItem, setItem };
}

type HarnessOptions = {
  readonly raw?: string | null;
  readonly read?: () => string | null | Promise<string | null>;
  readonly rootLayout?: SerializedDockview;
  readonly outputMoveGate?: Deferred<void>;
  readonly debounceMs?: number;
};

function createHarness(options: HarnessOptions = {}) {
  const order: string[] = [];
  const fakeRoot = createFakeRoot(options.rootLayout, order);
  const fakePort = createFakePort(order, options.outputMoveGate);
  const logsController = createLogsDockviewRuntime();
  const storage = createStorage(order, options.raw ?? null);
  const controller = createWorkbenchLayoutController({
    dockviewRead: fakePort.read,
    dockviewControl: fakePort.control,
    internal: fakePort.internal,
    logsRead: {
      subscribe: (listener) => logsController.subscribe(listener),
      getLatestSnapshot: () => logsController.getLatestSnapshot(),
    },
    logsControl: {
      beginRestore: () => logsController.beginRestore(),
      stageRestore: (epoch, layout) => logsController.stageRestore(epoch, layout),
      captureBoundSnapshot: () => logsController.captureBoundSnapshot(),
      resetToDefault: () => logsController.resetToDefault(),
    },
    storage,
    ...(options.read ? { read: options.read } : {}),
    debounceMs: options.debounceMs ?? 25,
  });
  return {
    order,
    root: fakeRoot,
    fakePort,
    logsController,
    storage,
    controller,
  };
}

async function bindAndHydrate(harness: ReturnType<typeof createHarness>): Promise<void> {
  harness.controller.bind(harness.root.api, "main");
  await harness.controller.whenHydrated();
}

async function flushMicrotasks(): Promise<void> {
  for (let index = 0; index < 12; index += 1) await Promise.resolve();
}

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.clearAllTimers();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("WorkbenchLayoutController hydration", () => {
  it("does not let stale hydration overwrite reset", async () => {
    const read = createDeferred<string | null>();
    const harness = createHarness({ read: () => read.promise });
    harness.controller.bind(harness.root.api, "main");

    const resetEpoch = harness.controller.beginLayoutReset();
    harness.controller.completeLayoutReset(resetEpoch);
    const currentHydration = harness.controller.whenHydrated();
    read.resolve(storedPayload(rootLayout("saved-logs", "logs")));
    await currentHydration;

    expect(harness.root.fromJSON).not.toHaveBeenCalled();
    expect(harness.fakePort.layoutEnsureView.mock.calls.map(([request]) => request.viewId)).toEqual(
      [
        "project",
        "data",
        "nodes",
        "commands",
        "details",
        "assistant",
        "logs",
        "output",
        "diagnostics",
      ],
    );
  });

  it("installs startup defaults behind hydration before queued application work", async () => {
    const harness = createHarness();
    harness.controller.bind(harness.root.api, "main");
    const externalOpen = harness.fakePort.port.openEditor({
      resourceRef: "events/queued-during-hydration.yssbi-event",
      resourceKind: "event",
      title: "Queued event",
      pinned: false,
      mode: "new-instance",
    });
    let hydrated = false;
    const hydration = harness.controller.whenHydrated().then(() => {
      hydrated = true;
    });

    await hydration;
    await externalOpen;

    expect(harness.fakePort.installHydrationLayout).toHaveBeenCalledOnce();
    expect(harness.fakePort.runLayoutTransaction).not.toHaveBeenCalled();
    expect(harness.fakePort.ensureCentralGroup).not.toHaveBeenCalled();
    expect(harness.fakePort.ensureView).not.toHaveBeenCalled();
    expect(harness.fakePort.configureEdge).not.toHaveBeenCalled();
    expect(harness.fakePort.move).not.toHaveBeenCalled();
    expect(harness.fakePort.layoutOperationHydrationStates).toHaveLength(22);
    expect(harness.fakePort.layoutOperationHydrationStates.every((state) => state === false)).toBe(
      true,
    );
    expect(harness.fakePort.layoutEnsureView.mock.calls.map(([request]) => request.viewId)).toEqual(
      [
        "project",
        "data",
        "nodes",
        "commands",
        "details",
        "assistant",
        "logs",
        "output",
        "diagnostics",
      ],
    );
    expect(harness.fakePort.layoutConfigureEdge.mock.calls.map(([request]) => request)).toEqual([
      { position: "left", size: 292, collapsed: false, headerPosition: "left" },
      { position: "right", size: 320, collapsed: false, headerPosition: "right" },
      { position: "bottom", size: 200, collapsed: false, headerPosition: "bottom" },
    ]);
    expect(harness.fakePort.layoutMove.mock.calls.map(([request]) => request)).toEqual([
      { panelInstanceId: "view:details", groupId: "edge-right", index: 0, activate: false },
      { panelInstanceId: "view:project", groupId: "edge-left", index: 0 },
      { panelInstanceId: "view:data", groupId: "edge-left", index: 1 },
      { panelInstanceId: "view:nodes", groupId: "edge-left", index: 2 },
      { panelInstanceId: "view:commands", groupId: "edge-left", index: 3 },
      { panelInstanceId: "view:assistant", groupId: "edge-right", index: 1, activate: false },
      { panelInstanceId: "view:logs", groupId: "edge-bottom", index: 0 },
      { panelInstanceId: "view:output", groupId: "edge-bottom", index: 1 },
      { panelInstanceId: "view:diagnostics", groupId: "edge-bottom", index: 2 },
    ]);
    expect(harness.order.indexOf("layout.move:view:output")).toBeLessThan(
      harness.order.indexOf("internal.installHydrationLayout:applied"),
    );
    expect(harness.order.indexOf("internal.installHydrationLayout:applied")).toBeLessThan(
      harness.order.indexOf("internal.completeHydration"),
    );
    expect(harness.order.indexOf("internal.completeHydration")).toBeLessThan(
      harness.order.indexOf("port.openEditor:events/queued-during-hydration.yssbi-event"),
    );
    expect(harness.logsController.getLatestSnapshot()).toEqual(createDefaultLogsDockviewLayout());
    expect(hydrated).toBe(true);
  });

  it("uses the ordinary transaction for an empty already-hydrated reset", async () => {
    const harness = createHarness();
    await bindAndHydrate(harness);
    harness.fakePort.installHydrationLayout.mockClear();
    harness.fakePort.runLayoutTransaction.mockClear();
    harness.fakePort.completeHydration.mockClear();
    harness.fakePort.layoutOperationHydrationStates.length = 0;

    const resetEpoch = harness.controller.beginLayoutReset();
    harness.controller.completeLayoutReset(resetEpoch);
    await harness.controller.whenHydrated();

    expect(harness.fakePort.installHydrationLayout).not.toHaveBeenCalled();
    expect(harness.fakePort.runLayoutTransaction).toHaveBeenCalledOnce();
    expect(harness.fakePort.completeHydration).not.toHaveBeenCalled();
    expect(harness.fakePort.layoutOperationHydrationStates).toHaveLength(22);
    expect(harness.fakePort.layoutOperationHydrationStates.every((state) => state === true)).toBe(
      true,
    );
  });

  it("preserves valid Logs while replacing only an invalid root with defaults", async () => {
    const savedLogs = logsLayout("execution");
    const harness = createHarness({ raw: storedPayload(invalidRootLayout(), savedLogs) });

    await bindAndHydrate(harness);

    expect(harness.root.fromJSON).not.toHaveBeenCalled();
    expect(harness.fakePort.layoutEnsureCentralGroup).toHaveBeenCalledOnce();
    expect(harness.logsController.getLatestSnapshot()).toEqual(savedLogs);
  });

  it("restores a valid root while replacing only invalid Logs with defaults", async () => {
    const savedRoot = rootLayout("saved-logs", "logs");
    const harness = createHarness({ raw: storedPayload(savedRoot, invalidLogsLayout()) });

    await bindAndHydrate(harness);

    expect(harness.root.fromJSON).toHaveBeenCalledOnce();
    expect(harness.root.fromJSON).toHaveBeenCalledWith(savedRoot, { reuseExistingPanels: true });
    expect(harness.fakePort.ensureCentralGroup).not.toHaveBeenCalled();
    expect(harness.logsController.getLatestSnapshot()).toEqual(createDefaultLogsDockviewLayout());
  });

  it("installs the permanent Details sidebar while restoring an older root layout", async () => {
    const savedRoot = rootLayout("saved-logs", "logs");
    const harness = createHarness({ raw: storedPayload(savedRoot) });

    await bindAndHydrate(harness);

    expect(harness.root.fromJSON).toHaveBeenCalledOnce();
    expect(harness.fakePort.installHydrationLayout).toHaveBeenCalledOnce();
    expect(harness.fakePort.layoutEnsureView).toHaveBeenCalledWith({
      viewId: "details",
      title: "Details",
    });
    expect(harness.fakePort.layoutConfigureEdge).toHaveBeenCalledWith({
      position: "right",
      size: 320,
      collapsed: false,
      headerPosition: "right",
    });
    expect(harness.fakePort.layoutMove).toHaveBeenCalledWith({
      panelInstanceId: "view:details",
      groupId: "edge-right",
      index: 0,
      activate: false,
    });
  });

  it("refuses startup restore when the bound root is not empty", async () => {
    const existing = rootLayout("existing-tool");
    const saved = rootLayout("saved-logs", "logs");
    const harness = createHarness({
      raw: storedPayload(saved),
      rootLayout: existing,
    });

    await bindAndHydrate(harness);

    expect(harness.root.fromJSON).not.toHaveBeenCalled();
    expect(harness.fakePort.ensureCentralGroup).not.toHaveBeenCalled();
    expect(harness.fakePort.internal.completeHydration).toHaveBeenCalledOnce();
  });

  it("starts root and Logs subscriptions only after hydration", async () => {
    const read = createDeferred<string | null>();
    const harness = createHarness({ read: () => read.promise });
    const logsSubscribe = vi.spyOn(harness.logsController, "subscribe");
    harness.controller.bind(harness.root.api, "main");

    expect(harness.fakePort.subscribe).not.toHaveBeenCalled();
    expect(logsSubscribe).not.toHaveBeenCalled();

    read.resolve(storedPayload());
    await harness.controller.whenHydrated();

    expect(harness.fakePort.subscribe).toHaveBeenCalledOnce();
    expect(logsSubscribe).toHaveBeenCalledOnce();
  });
});

describe("WorkbenchLayoutController persistence", () => {
  it("writes the whole current payload after a nested-only change", async () => {
    const harness = createHarness({ raw: storedPayload(), debounceMs: 20 });
    const serializedRoot = rootLayout("serialized-output", "output");
    harness.fakePort.setSerialized(serializedRoot);
    await bindAndHydrate(harness);
    harness.storage.setItem.mockClear();
    harness.fakePort.serialize.mockClear();

    const changedLogs = logsLayout("ui");
    const logsEpoch = harness.logsController.beginRestore();
    expect(harness.logsController.stageRestore(logsEpoch, changedLogs)).toBe("staged");
    expect(harness.storage.setItem).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(20);

    expect(harness.fakePort.serialize).toHaveBeenCalledOnce();
    expect(harness.storage.setItem).toHaveBeenCalledOnce();
    const [key, raw] = harness.storage.setItem.mock.calls[0]!;
    expect(key).toBe("yssbi-workbench-layout:main");
    expect(JSON.parse(raw)).toEqual({
      root: serializedRoot,
      nested: { logs: changedLogs },
    });
  });

  it("drops an overlapping normal write after its restore epoch becomes stale", async () => {
    const harness = createHarness({ raw: storedPayload(), debounceMs: 20 });
    const staleSerialize = createDeferred<SerializedDockview>();
    harness.fakePort.serialize.mockImplementationOnce(() => staleSerialize.promise);
    await bindAndHydrate(harness);
    harness.storage.setItem.mockClear();

    const logsEpoch = harness.logsController.beginRestore();
    harness.logsController.stageRestore(logsEpoch, logsLayout("graph"));
    vi.advanceTimersByTime(20);
    await flushMicrotasks();
    expect(harness.fakePort.serialize).toHaveBeenCalledOnce();

    const resetEpoch = harness.controller.beginLayoutReset();
    harness.controller.completeLayoutReset(resetEpoch);
    await harness.controller.whenHydrated();
    staleSerialize.resolve(rootLayout("stale-logs", "logs"));
    await flushMicrotasks();
    expect(harness.storage.setItem).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(20);
    expect(harness.storage.setItem).toHaveBeenCalledOnce();
  });

  it("waits for current hydration and FIFO idle before a direct close-time write", async () => {
    const read = createDeferred<string | null>();
    const idle = createDeferred<void>();
    const harness = createHarness({ read: () => read.promise });
    harness.fakePort.setIdle(idle.promise);
    vi.spyOn(harness.logsController, "captureBoundSnapshot").mockImplementation(() => {
      harness.order.push("logs.capture");
    });
    harness.controller.bind(harness.root.api, "main");
    harness.order.length = 0;

    const flush = harness.controller.flushBeforeWindowClose();
    await flushMicrotasks();
    expect(harness.fakePort.internal.whenIdle).not.toHaveBeenCalled();
    expect(harness.storage.setItem).not.toHaveBeenCalled();

    read.resolve(storedPayload());
    await harness.controller.whenHydrated();
    await flushMicrotasks();
    expect(harness.fakePort.internal.whenIdle).toHaveBeenCalledOnce();
    expect(harness.root.toJSON).not.toHaveBeenCalled();

    idle.resolve(undefined);
    await flush;

    expect(harness.fakePort.serialize).not.toHaveBeenCalled();
    expect(harness.order).toEqual([
      "root.fromJSON",
      "internal.installHydrationLayout:start",
      "layout.ensureView:details",
      "layout.configureEdge:right",
      "layout.move:view:details",
      "internal.completeHydration",
      "port.subscribe",
      "internal.whenIdle",
      "logs.capture",
      "root.toJSON",
      "storage.setItem",
    ]);
  });

  it("writes direct root and Logs snapshots before disposing a hydrated binding", async () => {
    const harness = createHarness({ raw: storedPayload() });
    vi.spyOn(harness.logsController, "captureBoundSnapshot").mockImplementation(() => {
      harness.order.push("logs.capture");
    });
    await bindAndHydrate(harness);
    harness.order.length = 0;
    harness.storage.setItem.mockClear();
    harness.root.toJSON.mockClear();
    harness.fakePort.serialize.mockClear();

    harness.controller.unbind(harness.root.api);

    expect(harness.fakePort.serialize).not.toHaveBeenCalled();
    expect(harness.root.toJSON).toHaveBeenCalledOnce();
    expect(harness.storage.setItem).toHaveBeenCalledOnce();
    expect(harness.order.indexOf("logs.capture")).toBeLessThan(
      harness.order.indexOf("root.toJSON"),
    );
    expect(harness.order.indexOf("storage.setItem")).toBeLessThan(
      harness.order.indexOf("port.unsubscribe"),
    );
    expect(harness.order.indexOf("storage.setItem")).toBeLessThan(
      harness.order.indexOf("internal.unbind"),
    );
  });

  it("scrubs the last bound project root before a new empty root restores it", async () => {
    const previousRoot = projectRootLayout();
    const savedLogs = logsLayout("execution");
    const harness = createHarness({
      raw: storedPayload(previousRoot, savedLogs),
      rootLayout: previousRoot,
    });
    await bindAndHydrate(harness);

    harness.controller.unbind(harness.root.api);
    const writesBeforeReplacement = harness.storage.setItem.mock.calls.length;
    harness.order.length = 0;

    harness.controller.invalidateForProjectReplacement();

    expect(harness.order[0]).toBe("internal.invalidatePendingOperations");
    expect(harness.fakePort.internal.invalidatePendingOperations).toHaveBeenCalledOnce();
    expect(harness.storage.setItem).toHaveBeenCalledTimes(writesBeforeReplacement + 1);
    const replacementRoot = createFakeRoot(undefined, harness.order);
    harness.controller.bind(replacementRoot.api, "main");
    await harness.controller.whenHydrated();

    expect(replacementRoot.fromJSON).toHaveBeenCalledOnce();
    const restoredRoot = replacementRoot.fromJSON.mock.calls[0]![0];
    expect(Object.keys(restoredRoot.panels)).toEqual([
      "project-stable",
      "nodes-stable",
      "data-stable",
      "commands-stable",
      "logs-stable",
      "output-stable",
    ]);
    expect(restoredRoot.grid.root).toEqual({ type: "branch", data: [] });
    expect(restoredRoot.grid).not.toHaveProperty("maximizedNode");
    expect(restoredRoot.edgeGroups).toEqual(previousRoot.edgeGroups);
    expect(restoredRoot.activeGroup).toBe("bottom-tools");
    expect(harness.logsController.getLatestSnapshot()).toEqual(savedLogs);
  });

  it("does not overwrite storage when unbound before hydration succeeds", async () => {
    const read = createDeferred<string | null>();
    const harness = createHarness({ read: () => read.promise });
    harness.controller.bind(harness.root.api, "main");
    const pendingHydration = harness.controller.whenHydrated();

    harness.controller.unbind(harness.root.api);
    await pendingHydration;
    read.resolve(storedPayload());
    await flushMicrotasks();

    expect(harness.storage.setItem).not.toHaveBeenCalled();
    expect(harness.root.toJSON).not.toHaveBeenCalled();
    expect(harness.root.fromJSON).not.toHaveBeenCalled();
    expect(harness.fakePort.ensureCentralGroup).not.toHaveBeenCalled();
    expect(harness.fakePort.internal.unbind).toHaveBeenCalledWith(harness.root.api);
  });
});

describe("WorkbenchLayoutController project readiness generations", () => {
  it("retains a resources-ready callback marked before the first bind", async () => {
    const read = createDeferred<string | null>();
    const harness = createHarness({ read: () => read.promise });
    const callback = vi.fn();

    harness.controller.markProjectResourcesReady(callback);
    harness.controller.bind(harness.root.api, "main");
    await flushMicrotasks();
    expect(callback).not.toHaveBeenCalled();

    read.resolve(storedPayload());
    await harness.controller.whenHydrated();
    await flushMicrotasks();

    expect(callback).toHaveBeenCalledOnce();
    expect(harness.controller.projectResourcesReady).toBe(true);
  });

  it("reruns the authoritative callback after a same-project rebind hydrates", async () => {
    const rebindRead = createDeferred<string | null>();
    const read = vi
      .fn()
      .mockResolvedValueOnce(storedPayload())
      .mockReturnValueOnce(rebindRead.promise);
    const harness = createHarness({ read });
    await bindAndHydrate(harness);
    const callback = vi.fn();
    harness.controller.markProjectResourcesReady(callback);
    await flushMicrotasks();
    expect(callback).toHaveBeenCalledOnce();
    expect(harness.controller.projectResourcesReady).toBe(true);

    harness.controller.unbind(harness.root.api);
    harness.controller.bind(harness.root.api, "main");
    await flushMicrotasks();
    expect(callback).toHaveBeenCalledOnce();
    expect(harness.controller.projectResourcesReady).toBe(false);

    rebindRead.resolve(storedPayload());
    await harness.controller.whenHydrated();
    await flushMicrotasks();

    expect(callback).toHaveBeenCalledTimes(2);
    expect(harness.controller.projectResourcesReady).toBe(true);
  });

  it("runs a resources-first callback only after root hydration", async () => {
    const read = createDeferred<string | null>();
    const harness = createHarness({ read: () => read.promise });
    const callback = vi.fn(async (context: { isCurrent(): boolean }) => {
      expect(context.isCurrent()).toBe(true);
      await Promise.resolve();
      expect(context.isCurrent()).toBe(true);
    });
    harness.controller.bind(harness.root.api, "main");

    harness.controller.markProjectResourcesReady(callback);
    await flushMicrotasks();
    expect(callback).not.toHaveBeenCalled();
    expect(harness.controller.projectResourcesReady).toBe(false);

    read.resolve(storedPayload());
    await harness.controller.whenHydrated();
    await flushMicrotasks();

    expect(callback).toHaveBeenCalledOnce();
    expect(harness.controller.projectResourcesReady).toBe(true);
  });

  it("runs a root-first callback when resources subsequently become ready", async () => {
    const harness = createHarness({ raw: storedPayload() });
    await bindAndHydrate(harness);
    const callback = vi.fn(async (context: { isCurrent(): boolean }) => {
      expect(context.isCurrent()).toBe(true);
    });

    harness.controller.markProjectResourcesReady(callback);
    await flushMicrotasks();

    expect(callback).toHaveBeenCalledOnce();
    expect(harness.controller.projectResourcesReady).toBe(true);
  });

  it("replaces the authoritative callback when resources are marked again", async () => {
    const harness = createHarness({ raw: storedPayload() });
    await bindAndHydrate(harness);
    const firstCallback = vi.fn();
    const replacementCallback = vi.fn();

    harness.controller.markProjectResourcesReady(firstCallback);
    await flushMicrotasks();
    harness.controller.markProjectResourcesReady(replacementCallback);
    await flushMicrotasks();

    const resetEpoch = harness.controller.beginLayoutReset();
    harness.controller.completeLayoutReset(resetEpoch);
    await harness.controller.whenHydrated();
    await flushMicrotasks();

    expect(firstCallback).toHaveBeenCalledOnce();
    expect(replacementCallback).toHaveBeenCalledTimes(2);
    expect(harness.controller.projectResourcesReady).toBe(true);
  });

  it("makes a late startup read and old callback stale after project replacement", async () => {
    const read = createDeferred<string | null>();
    const harness = createHarness({ read: () => read.promise });
    const staleCallback = vi.fn();
    harness.controller.bind(harness.root.api, "main");
    harness.controller.markProjectResourcesReady(staleCallback);

    harness.controller.invalidateForProjectReplacement();
    const replacementHydration = harness.controller.whenHydrated();
    read.resolve(storedPayload(rootLayout("saved-logs", "logs")));
    await replacementHydration;
    await flushMicrotasks();

    expect(harness.root.fromJSON).not.toHaveBeenCalled();
    expect(harness.fakePort.layoutEnsureCentralGroup).toHaveBeenCalledOnce();
    expect(staleCallback).not.toHaveBeenCalled();
    expect(harness.controller.projectResourcesReady).toBe(false);

    const currentCallback = vi.fn();
    harness.controller.markProjectResourcesReady(currentCallback);
    await flushMicrotasks();
    expect(currentCallback).toHaveBeenCalledOnce();
    expect(harness.controller.projectResourcesReady).toBe(true);
  });

  it("makes an in-flight callback stale after project replacement", async () => {
    const harness = createHarness({ raw: storedPayload() });
    await bindAndHydrate(harness);
    const callbackGate = createDeferred<void>();
    let currentAfterAwait: boolean | undefined;
    const callback = vi.fn(async (context: { isCurrent(): boolean }) => {
      expect(context.isCurrent()).toBe(true);
      await callbackGate.promise;
      currentAfterAwait = context.isCurrent();
    });
    harness.controller.markProjectResourcesReady(callback);
    await flushMicrotasks();
    expect(callback).toHaveBeenCalledOnce();

    harness.controller.invalidateForProjectReplacement();
    callbackGate.resolve(undefined);
    await harness.controller.whenHydrated();
    await flushMicrotasks();

    expect(currentAfterAwait).toBe(false);
    expect(callback).toHaveBeenCalledOnce();
    expect(harness.controller.projectResourcesReady).toBe(false);
  });

  it("reruns the authoritative callback after layout reset hydrates", async () => {
    const harness = createHarness({ raw: storedPayload() });
    await bindAndHydrate(harness);
    const callback = vi.fn();
    harness.controller.markProjectResourcesReady(callback);
    await flushMicrotasks();
    expect(callback).toHaveBeenCalledOnce();
    expect(harness.controller.projectResourcesReady).toBe(true);

    const resetEpoch = harness.controller.beginLayoutReset();
    expect(harness.controller.projectResourcesReady).toBe(false);
    harness.controller.completeLayoutReset(resetEpoch);
    await harness.controller.whenHydrated();
    await flushMicrotasks();

    expect(callback).toHaveBeenCalledTimes(2);
    expect(harness.controller.projectResourcesReady).toBe(true);
  });
});
