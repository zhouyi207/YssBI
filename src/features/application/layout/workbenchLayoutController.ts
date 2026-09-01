import type { DockviewApi, SerializedDockview } from "dockview-react";

import { DEFAULT_LOGS_DOCKVIEW_LAYOUT } from "@/features/core/dockview/logsDockviewLayout";
import { createLogsDockviewRuntime } from "@/features/core/dockview/logsRuntime";
import { logsDockviewRead, type LogsDockviewRead } from "@/features/core/dockview/logsRead";
import {
  logsDockviewControl,
  type LogsDockviewControl,
} from "@/features/core/dockview/logsControl";
import {
  createPersistedWorkbenchLayout,
  parsePersistedWorkbenchLayout,
  scrubProjectScopedRootLayout,
  workbenchLayoutStorageKey,
} from "@/features/core/dockview/workbenchLayoutPersistence";
import {
  WORKBENCH_ACTIVITY_DEFAULT_ORDER,
  WORKBENCH_EDGE_SIZES,
} from "@/features/core/dockview/workbenchDockviewDefaults";
import {
  createWorkbenchDockviewRuntime,
  workbenchDockviewInternal,
  type WorkbenchDockviewInternal,
  type WorkbenchLayoutTransaction,
} from "@/features/core/dockview/workbenchDockviewInternal";
import {
  workbenchDockviewRead,
  type WorkbenchDockviewRead,
  type WorkbenchPanelInfo,
} from "@/features/core/dockview/workbenchRead";
import {
  workbenchDockviewControl,
  type WorkbenchDockviewControl,
} from "@/features/core/dockview/workbenchControl";

export interface ProjectResourcesReadyContext {
  isCurrent(): boolean;
}

export interface WorkbenchLayoutController {
  readonly projectResourcesReady: boolean;
  bind(api: DockviewApi, windowLabel: string): void;
  unbind(api?: DockviewApi): void;
  whenHydrated(): Promise<void>;
  flushBeforeWindowClose(): Promise<void>;
  beginLayoutReset(): number;
  completeLayoutReset(epoch: number): void;
  invalidateForProjectReplacement(): void;
  markProjectResourcesReady(
    callback: (context: ProjectResourcesReadyContext) => void | Promise<void>,
  ): void;
}

type LayoutStorage = Pick<Storage, "getItem" | "setItem">;

type LayoutReader = (key: string) => string | null | Promise<string | null>;

type ControllerDependencies = {
  readonly dockviewRead?: WorkbenchDockviewRead;
  readonly dockviewControl?: WorkbenchDockviewControl;
  readonly internal?: WorkbenchDockviewInternal;
  readonly logsRead?: LogsDockviewRead;
  readonly logsControl?: LogsDockviewControl;
  readonly storage?: LayoutStorage;
  readonly read?: LayoutReader;
  readonly debounceMs?: number;
};

type BoundRoot = {
  readonly api: DockviewApi;
  readonly key: string;
  readonly generation: number;
  readonly internalHydrationEpoch: number;
};

type HydrationCycle = {
  readonly epoch: number;
  readonly bindingGeneration: number;
  readonly internalHydrationEpoch: number;
  readonly promise: Promise<void>;
  readonly resolve: () => void;
  readonly reject: (error: unknown) => void;
  settled: boolean;
  successful: boolean;
  completing: boolean;
};

type PendingResourcesReady = {
  readonly requestId: number;
  readonly projectGeneration: number;
  readonly restoreEpoch: number;
  readonly bindingGeneration: number;
  readonly callback: (context: ProjectResourcesReadyContext) => void | Promise<void>;
};

const DEFAULT_PERSISTENCE_DEBOUNCE_MS = 250;

const browserStorage: LayoutStorage = {
  getItem(key) {
    return typeof localStorage === "undefined" ? null : localStorage.getItem(key);
  },
  setItem(key, value) {
    if (typeof localStorage !== "undefined") localStorage.setItem(key, value);
  },
};

function createHydrationCycle(epoch: number, bound: BoundRoot): HydrationCycle {
  let resolvePromise!: () => void;
  let rejectPromise!: (error: unknown) => void;
  const promise = new Promise<void>((resolve, reject) => {
    resolvePromise = resolve;
    rejectPromise = reject;
  });
  void promise.catch(() => undefined);
  return {
    epoch,
    bindingGeneration: bound.generation,
    internalHydrationEpoch: bound.internalHydrationEpoch,
    promise,
    resolve: resolvePromise,
    reject: rejectPromise,
    settled: false,
    successful: false,
    completing: false,
  };
}

function rootIsEmpty(api: DockviewApi): boolean {
  return api.panels.length === 0;
}

function viewRequest(viewId: "diagnostics") {
  return { viewId, title: "Diagnostics" } as const;
}

const DETAILS_VIEW_REQUEST = {
  viewId: "details",
  title: "Details",
} as const;

const ASSISTANT_VIEW_REQUEST = {
  viewId: "assistant",
  title: "Assistant",
} as const;

function persistedRightSidebarSize(transaction: WorkbenchLayoutTransaction): number {
  const right = transaction.serialize().edgeGroups?.right;
  return typeof right?.size === "number" ? right.size : WORKBENCH_EDGE_SIZES.right;
}

function configurePermanentDetailsSidebar(
  transaction: WorkbenchLayoutTransaction,
  details: WorkbenchPanelInfo,
  size = persistedRightSidebarSize(transaction),
): string {
  const right = transaction.configureEdge({
    position: "right",
    size,
    collapsed: false,
    headerPosition: "right",
  });
  transaction.move({
    panelInstanceId: details.panelInstanceId,
    groupId: right.groupId,
    index: 0,
    activate: false,
  });
  return right.groupId;
}

function ensurePermanentDetailsSidebar(transaction: WorkbenchLayoutTransaction): void {
  const activePanelInstanceId = transaction.getActivePanel()?.panelInstanceId;
  const details = transaction.ensureView(DETAILS_VIEW_REQUEST);
  configurePermanentDetailsSidebar(transaction, details);
  if (activePanelInstanceId && transaction.getPanel(activePanelInstanceId)) {
    transaction.activate(activePanelInstanceId);
  }
}

function ensureDiagnosticsViewAfterRestore(transaction: WorkbenchLayoutTransaction): void {
  const existing = transaction
    .listPanels()
    .find((panel) => panel.metadata.role === "view" && panel.metadata.viewId === "diagnostics");
  if (existing) return;

  const activePanelInstanceId = transaction.getActivePanel()?.panelInstanceId;
  const diagnostics = transaction.ensureView(viewRequest("diagnostics"));
  const output = transaction
    .listPanels()
    .find((panel) => panel.metadata.role === "view" && panel.metadata.viewId === "output");
  if (output) {
    const outputGroupPanels = transaction.listGroupPanels(output.groupId);
    const outputIndex = outputGroupPanels.findIndex(
      (panel) => panel.panelInstanceId === output.panelInstanceId,
    );
    if (outputIndex >= 0) {
      transaction.move({
        panelInstanceId: diagnostics.panelInstanceId,
        groupId: output.groupId,
        index: outputIndex + 1,
        activate: false,
      });
    }
  }
  if (activePanelInstanceId && transaction.getPanel(activePanelInstanceId)) {
    transaction.activate(activePanelInstanceId);
  }
}

function installDefaultRootLayout(transaction: WorkbenchLayoutTransaction): void {
  transaction.ensureCentralGroup();
  const activityPanels = WORKBENCH_ACTIVITY_DEFAULT_ORDER.map((viewId) =>
    transaction.ensureView({ viewId, title: viewId[0].toUpperCase() + viewId.slice(1) }),
  );
  const details = transaction.ensureView(DETAILS_VIEW_REQUEST);
  const assistant = transaction.ensureView(ASSISTANT_VIEW_REQUEST);
  const logs = transaction.ensureView({ viewId: "logs", title: "Logs" });
  const output = transaction.ensureView({ viewId: "output", title: "Output" });
  const diagnostics = transaction.ensureView(viewRequest("diagnostics"));
  const left = transaction.configureEdge({
    position: "left",
    size: WORKBENCH_EDGE_SIZES.left,
    collapsed: false,
    headerPosition: "left",
  });
  const rightGroupId = configurePermanentDetailsSidebar(transaction, details);
  const bottom = transaction.configureEdge({
    position: "bottom",
    size: WORKBENCH_EDGE_SIZES.bottom,
    collapsed: false,
    headerPosition: "bottom",
  });
  activityPanels.forEach((panel, index) => {
    transaction.move({
      panelInstanceId: panel.panelInstanceId,
      groupId: left.groupId,
      index,
    });
  });
  transaction.move({
    panelInstanceId: assistant.panelInstanceId,
    groupId: rightGroupId,
    index: 1,
    activate: false,
  });
  transaction.move({
    panelInstanceId: logs.panelInstanceId,
    groupId: bottom.groupId,
    index: 0,
  });
  transaction.move({
    panelInstanceId: output.panelInstanceId,
    groupId: bottom.groupId,
    index: 1,
  });
  transaction.move({
    panelInstanceId: diagnostics.panelInstanceId,
    groupId: bottom.groupId,
    index: 2,
  });
  const project = activityPanels.find(
    (panel) => panel.metadata.role === "view" && panel.metadata.viewId === "project",
  );
  if (project) transaction.activate(project.panelInstanceId);
}

function parseStoredLayout(raw: string | null) {
  if (raw === null) return null;
  try {
    return parsePersistedWorkbenchLayout(JSON.parse(raw));
  } catch {
    return null;
  }
}

function scrubStoredProjectRoot(storage: LayoutStorage, key: string): void {
  try {
    const raw = storage.getItem(key);
    if (raw === null) return;
    const candidate: unknown = JSON.parse(raw);
    const parsed = parsePersistedWorkbenchLayout(candidate);
    if (!parsed || parsed.root.status !== "valid") return;

    storage.setItem(
      key,
      JSON.stringify({
        ...(candidate as Record<string, unknown>),
        root: scrubProjectScopedRootLayout(parsed.root.value),
      }),
    );
  } catch {
    // Startup hydration owns fallback for unreadable or unwritable snapshots.
  }
}

export function createWorkbenchLayoutController(
  dependencies: ControllerDependencies = {},
): WorkbenchLayoutController {
  const hasInjectedRuntime =
    dependencies.dockviewRead !== undefined ||
    dependencies.dockviewControl !== undefined ||
    dependencies.internal !== undefined;
  if (
    hasInjectedRuntime &&
    (dependencies.dockviewRead === undefined ||
      dependencies.dockviewControl === undefined ||
      dependencies.internal === undefined)
  ) {
    throw new Error("read, control, and internal must be injected together");
  }

  const isolated = hasInjectedRuntime ? undefined : createWorkbenchDockviewRuntime();
  const read = dependencies.dockviewRead ?? isolated!.read;
  const control = dependencies.dockviewControl ?? isolated!.control;
  const internal = dependencies.internal ?? isolated!.internal;
  const hasInjectedLogs =
    dependencies.logsRead !== undefined || dependencies.logsControl !== undefined;
  if (
    hasInjectedLogs &&
    (dependencies.logsRead === undefined || dependencies.logsControl === undefined)
  ) {
    throw new Error("logsRead and logsControl must be injected together");
  }
  const isolatedLogs = hasInjectedLogs ? undefined : createLogsDockviewRuntime();
  const logsRead =
    dependencies.logsRead ??
    ({
      subscribe: isolatedLogs!.subscribe,
      getLatestSnapshot: isolatedLogs!.getLatestSnapshot,
    } satisfies LogsDockviewRead);
  const logsControl =
    dependencies.logsControl ??
    ({
      beginRestore: isolatedLogs!.beginRestore,
      stageRestore: isolatedLogs!.stageRestore,
      captureBoundSnapshot: isolatedLogs!.captureBoundSnapshot,
      resetToDefault: isolatedLogs!.resetToDefault,
    } satisfies LogsDockviewControl);
  const storage = dependencies.storage ?? browserStorage;
  const storageRead = dependencies.read ?? ((key: string) => storage.getItem(key));
  const debounceMs = dependencies.debounceMs ?? DEFAULT_PERSISTENCE_DEBOUNCE_MS;

  let bound: BoundRoot | undefined;
  let currentStorageKey: string | undefined;
  let bindingGeneration = 0;
  let restoreEpoch = 0;
  let currentCycle: HydrationCycle | undefined;
  let hydratedEpoch: number | undefined;

  let persistenceCycle: HydrationCycle | undefined;
  let persistenceDisposers: Array<() => void> = [];
  let persistenceTimer: ReturnType<typeof setTimeout> | undefined;
  let persistenceRequest = 0;
  let writeSuspensionDepth = 0;

  let projectGeneration = 0;
  let resourcesReadyRequest = 0;
  let pendingResourcesReady: PendingResourcesReady | undefined;
  let resourcesReady = false;

  const isCurrentCycle = (cycle: HydrationCycle): boolean =>
    currentCycle === cycle &&
    restoreEpoch === cycle.epoch &&
    bound?.generation === cycle.bindingGeneration;

  const isSuccessfullyHydrated = (cycle: HydrationCycle): boolean =>
    isCurrentCycle(cycle) && cycle.successful && cycle.settled && hydratedEpoch === cycle.epoch;

  const settleInvalidatedCycle = (): void => {
    const cycle = currentCycle;
    if (!cycle || cycle.settled) return;
    cycle.settled = true;
    cycle.resolve();
  };

  const invalidateScheduledWrites = (): void => {
    persistenceRequest += 1;
    if (persistenceTimer !== undefined) {
      clearTimeout(persistenceTimer);
      persistenceTimer = undefined;
    }
  };

  const pausePersistence = (): void => {
    invalidateScheduledWrites();
    persistenceCycle = undefined;
    const disposers = persistenceDisposers;
    persistenceDisposers = [];
    disposers.forEach((dispose) => dispose());
  };

  const advanceProjectGeneration = (): void => {
    projectGeneration += 1;
    resourcesReadyRequest += 1;
    pendingResourcesReady = undefined;
    resourcesReady = false;
  };

  const beginCycle = (currentBound: BoundRoot): HydrationCycle => {
    settleInvalidatedCycle();
    restoreEpoch += 1;
    hydratedEpoch = undefined;
    const cycle = createHydrationCycle(restoreEpoch, currentBound);
    currentCycle = cycle;
    return cycle;
  };

  const rebasePendingResourcesReady = (cycle: HydrationCycle): void => {
    const request = pendingResourcesReady;
    if (!request || request.projectGeneration !== projectGeneration) return;
    pendingResourcesReady = {
      ...request,
      restoreEpoch: cycle.epoch,
      bindingGeneration: cycle.bindingGeneration,
    };
  };

  const isCurrentResourcesRequest = (request: PendingResourcesReady): boolean =>
    request.requestId === resourcesReadyRequest &&
    request.projectGeneration === projectGeneration &&
    request.restoreEpoch === restoreEpoch &&
    request.bindingGeneration === bound?.generation &&
    currentCycle?.epoch === request.restoreEpoch &&
    isSuccessfullyHydrated(currentCycle);

  const runPendingResourcesReady = (): void => {
    const request = pendingResourcesReady;
    if (!request || !isCurrentResourcesRequest(request)) return;

    const context: ProjectResourcesReadyContext = {
      isCurrent: () => isCurrentResourcesRequest(request),
    };
    void (async () => {
      if (!context.isCurrent()) return;
      try {
        await request.callback(context);
      } catch {
        return;
      }
      if (context.isCurrent()) resourcesReady = true;
    })();
  };

  const writePayload = (currentBound: BoundRoot, root: SerializedDockview): void => {
    const payload = createPersistedWorkbenchLayout(root, logsRead.getLatestSnapshot());
    storage.setItem(currentBound.key, JSON.stringify(payload));
  };

  const writePersistedLayout = async (cycle: HydrationCycle, request: number): Promise<void> => {
    if (
      writeSuspensionDepth > 0 ||
      persistenceCycle !== cycle ||
      request !== persistenceRequest ||
      !isSuccessfullyHydrated(cycle)
    )
      return;

    const currentBound = bound;
    if (!currentBound) return;
    const root = await control.serialize();
    if (
      writeSuspensionDepth > 0 ||
      persistenceCycle !== cycle ||
      request !== persistenceRequest ||
      !isSuccessfullyHydrated(cycle) ||
      bound !== currentBound
    )
      return;

    writePayload(currentBound, root);
  };

  const schedulePersistence = (cycle: HydrationCycle): void => {
    if (writeSuspensionDepth > 0 || persistenceCycle !== cycle || !isSuccessfullyHydrated(cycle))
      return;

    invalidateScheduledWrites();
    const request = persistenceRequest;
    persistenceTimer = setTimeout(() => {
      persistenceTimer = undefined;
      void writePersistedLayout(cycle, request).catch(() => undefined);
    }, debounceMs);
  };

  const startPersistence = (cycle: HydrationCycle): void => {
    pausePersistence();
    persistenceCycle = cycle;
    const schedule = () => schedulePersistence(cycle);
    persistenceDisposers = [read.subscribe(schedule), logsRead.subscribe(schedule)];
  };

  const failCycle = (cycle: HydrationCycle, error: unknown): void => {
    if (!isCurrentCycle(cycle) || cycle.settled) return;
    pausePersistence();
    cycle.successful = false;
    cycle.settled = true;
    cycle.reject(error);
  };

  const finishCycle = (cycle: HydrationCycle, persistCurrentLayout: boolean): void => {
    if (!isCurrentCycle(cycle) || cycle.settled) return;
    cycle.successful = true;
    hydratedEpoch = cycle.epoch;
    try {
      startPersistence(cycle);
    } catch (error) {
      cycle.successful = false;
      hydratedEpoch = undefined;
      failCycle(cycle, error);
      return;
    }
    cycle.settled = true;
    cycle.resolve();
    if (persistCurrentLayout) schedulePersistence(cycle);
    runPendingResourcesReady();
  };

  const openHydrationGateAndFinish = (
    cycle: HydrationCycle,
    persistCurrentLayout: boolean,
  ): void => {
    if (!isCurrentCycle(cycle)) return;
    try {
      internal.completeHydration(cycle.internalHydrationEpoch);
      finishCycle(cycle, persistCurrentLayout);
    } catch (error) {
      failCycle(cycle, error);
    }
  };

  const finishRestoredRoot = async (cycle: HydrationCycle): Promise<void> => {
    try {
      internal.installHydrationLayout(cycle.internalHydrationEpoch, (transaction) => {
        ensurePermanentDetailsSidebar(transaction);
        ensureDiagnosticsViewAfterRestore(transaction);
      });
    } catch {
      // A restored root remains usable if additive view migrations fail.
    }
    openHydrationGateAndFinish(cycle, false);
  };

  const initializeRootDefaults = async (
    cycle: HydrationCycle,
    persistCurrentLayout: boolean,
  ): Promise<void> => {
    if (!isCurrentCycle(cycle) || cycle.completing) return;
    cycle.completing = true;
    const requiresHydrationInstall = !read.isHydrated;
    try {
      if (requiresHydrationInstall) {
        internal.installHydrationLayout(cycle.internalHydrationEpoch, installDefaultRootLayout);
      } else {
        await internal.runLayoutTransaction(installDefaultRootLayout);
      }
      if (!isCurrentCycle(cycle)) return;

      if (requiresHydrationInstall) {
        internal.completeHydration(cycle.internalHydrationEpoch);
        if (!isCurrentCycle(cycle)) return;
      }
      finishCycle(cycle, persistCurrentLayout);
    } catch (error) {
      failCycle(cycle, error);
    }
  };

  const stageLogsLayout = (
    cycle: HydrationCycle,
    logsRestoreEpoch: number,
    layout: SerializedDockview,
  ): boolean => {
    if (!isCurrentCycle(cycle)) return false;
    try {
      return (
        logsControl.stageRestore(logsRestoreEpoch, layout) !== "stale" && isCurrentCycle(cycle)
      );
    } catch {
      if (!isCurrentCycle(cycle)) return false;
      try {
        logsControl.resetToDefault();
        return isCurrentCycle(cycle);
      } catch (error) {
        failCycle(cycle, error);
        return false;
      }
    }
  };

  const hydrateStartup = async (cycle: HydrationCycle, logsRestoreEpoch: number): Promise<void> => {
    const currentBound = bound;
    if (!currentBound || currentBound.generation !== cycle.bindingGeneration) return;

    let raw: string | null;
    try {
      raw = await storageRead(currentBound.key);
    } catch {
      raw = null;
    }
    if (!isCurrentCycle(cycle) || bound !== currentBound) return;

    const parsed = parseStoredLayout(raw);
    const logsLayout =
      parsed?.logs.status === "valid" ? parsed.logs.value : DEFAULT_LOGS_DOCKVIEW_LAYOUT;
    if (!stageLogsLayout(cycle, logsRestoreEpoch, logsLayout)) return;

    if (parsed?.root.status !== "valid") {
      if (rootIsEmpty(currentBound.api)) {
        await initializeRootDefaults(cycle, false);
      } else {
        await finishRestoredRoot(cycle);
      }
      return;
    }

    if (!rootIsEmpty(currentBound.api)) {
      await finishRestoredRoot(cycle);
      return;
    }

    try {
      if (!isCurrentCycle(cycle) || !rootIsEmpty(currentBound.api)) return;
      currentBound.api.fromJSON(structuredClone(parsed.root.value), {
        reuseExistingPanels: true,
      });
    } catch {
      if (!isCurrentCycle(cycle)) return;
      if (rootIsEmpty(currentBound.api)) {
        await initializeRootDefaults(cycle, false);
      } else {
        await finishRestoredRoot(cycle);
      }
      return;
    }

    await finishRestoredRoot(cycle);
  };

  const controller: WorkbenchLayoutController = {
    get projectResourcesReady() {
      return resourcesReady;
    },

    bind(api, windowLabel) {
      const key = workbenchLayoutStorageKey(windowLabel);
      if (
        bound?.api === api &&
        bound.key === key &&
        currentCycle &&
        (!currentCycle.settled || currentCycle.successful)
      )
        return;
      if (bound) controller.unbind(bound.api);
      currentStorageKey = key;

      pausePersistence();
      const logsRestoreEpoch = logsControl.beginRestore();
      const internalHydrationEpoch = internal.beginHydration();
      bindingGeneration += 1;
      const nextBound: BoundRoot = {
        api,
        key,
        generation: bindingGeneration,
        internalHydrationEpoch,
      };
      bound = nextBound;
      const cycle = beginCycle(nextBound);
      rebasePendingResourcesReady(cycle);
      try {
        internal.bind(api);
      } catch (error) {
        failCycle(cycle, error);
        throw error;
      }
      void hydrateStartup(cycle, logsRestoreEpoch).catch((error) => {
        failCycle(cycle, error);
      });
    },

    unbind(api) {
      const currentBound = bound;
      if (!currentBound || (api !== undefined && api !== currentBound.api)) return;
      const cycle = currentCycle;
      const canPersist = cycle !== undefined && isSuccessfullyHydrated(cycle);

      writeSuspensionDepth += 1;
      invalidateScheduledWrites();
      try {
        if (canPersist) {
          try {
            logsControl.captureBoundSnapshot();
            if (isSuccessfullyHydrated(cycle) && bound === currentBound) {
              writePayload(currentBound, currentBound.api.toJSON());
            }
          } catch {
            // Unbind must still release the live Dockview API.
          }
        }
      } finally {
        writeSuspensionDepth -= 1;
        pausePersistence();
        settleInvalidatedCycle();
        restoreEpoch += 1;
        hydratedEpoch = undefined;
        currentCycle = undefined;
        logsControl.beginRestore();
        resourcesReady = false;
        internal.unbind(currentBound.api);
        if (bound === currentBound) bound = undefined;
      }
    },

    whenHydrated() {
      return currentCycle?.promise ?? Promise.resolve();
    },

    async flushBeforeWindowClose() {
      const cycle = currentCycle;
      const currentBound = bound;
      if (!cycle || !currentBound) return;
      await cycle.promise;
      if (!isSuccessfullyHydrated(cycle) || bound !== currentBound) return;

      writeSuspensionDepth += 1;
      invalidateScheduledWrites();
      try {
        await internal.whenIdle();
        if (!isSuccessfullyHydrated(cycle) || bound !== currentBound) return;
        logsControl.captureBoundSnapshot();
        if (!isSuccessfullyHydrated(cycle) || bound !== currentBound) return;
        const root = currentBound.api.toJSON();
        if (!isSuccessfullyHydrated(cycle) || bound !== currentBound) return;
        writePayload(currentBound, root);
      } finally {
        invalidateScheduledWrites();
        writeSuspensionDepth -= 1;
      }
    },

    beginLayoutReset() {
      pausePersistence();
      resourcesReady = false;
      logsControl.beginRestore();
      if (!bound) {
        settleInvalidatedCycle();
        restoreEpoch += 1;
        hydratedEpoch = undefined;
        currentCycle = undefined;
        return restoreEpoch;
      }
      const cycle = beginCycle(bound);
      rebasePendingResourcesReady(cycle);
      return cycle.epoch;
    },

    completeLayoutReset(epoch) {
      const cycle = currentCycle;
      const currentBound = bound;
      if (!cycle || !currentBound || cycle.epoch !== epoch || !isCurrentCycle(cycle)) return;
      if (rootIsEmpty(currentBound.api)) {
        void initializeRootDefaults(cycle, true);
      } else {
        openHydrationGateAndFinish(cycle, true);
      }
    },

    invalidateForProjectReplacement() {
      internal.invalidatePendingOperations();
      pausePersistence();
      advanceProjectGeneration();
      logsControl.beginRestore();
      if (!bound) {
        settleInvalidatedCycle();
        restoreEpoch += 1;
        hydratedEpoch = undefined;
        currentCycle = undefined;
        if (currentStorageKey !== undefined) {
          scrubStoredProjectRoot(storage, currentStorageKey);
        }
        return;
      }

      const cycle = beginCycle(bound);
      if (rootIsEmpty(bound.api)) {
        void initializeRootDefaults(cycle, true);
      } else {
        openHydrationGateAndFinish(cycle, true);
      }
    },

    markProjectResourcesReady(callback) {
      resourcesReady = false;
      const cycle = currentCycle;
      resourcesReadyRequest += 1;
      pendingResourcesReady = {
        requestId: resourcesReadyRequest,
        projectGeneration,
        restoreEpoch: cycle?.epoch ?? restoreEpoch,
        bindingGeneration: bound?.generation ?? bindingGeneration,
        callback,
      };
      runPendingResourcesReady();
    },
  };

  return controller;
}

export const workbenchLayoutController = createWorkbenchLayoutController({
  dockviewRead: workbenchDockviewRead,
  dockviewControl: workbenchDockviewControl,
  internal: workbenchDockviewInternal,
  logsRead: logsDockviewRead,
  logsControl: logsDockviewControl,
  storage: browserStorage,
});
