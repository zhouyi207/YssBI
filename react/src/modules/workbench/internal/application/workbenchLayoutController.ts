import type { IJsonModel } from "flexlayout-react";
import type { LayoutModelBinding } from "../layout/layoutModelBinding";
import { modelTabs } from "../layout/workbenchLayoutOperations";

import { DEFAULT_LOGS_LAYOUT } from "../layout/logsLayoutModel";
import { createLogsLayoutRuntime } from "../layout/logsRuntime";
import { logsLayoutRead, type LogsLayoutRead } from "../layout/logsRead";
import { logsLayoutControl, type LogsLayoutControl } from "../layout/logsControl";
import {
  createPersistedWorkbenchLayout,
  parseStoredWorkbenchLayout,
  scrubStoredProjectRoot,
  workbenchLayoutStorage,
  type WorkbenchLayoutStorage,
  workbenchLayoutStorageKey,
} from "../layout/workbenchLayoutPersistence";
import {
  installDefaultRootLayout,
  ensureRestoredRootPanels,
} from "../layout/workbenchLayoutDefaults";
import {
  createWorkbenchLayoutRuntime,
  workbenchLayoutInternal,
  type WorkbenchLayoutInternal,
} from "../layout/workbenchLayoutInternal";
import { workbenchLayoutRead, type WorkbenchLayoutRead } from "../layout/workbenchRead";
import { workbenchLayoutControl, type WorkbenchLayoutControl } from "../layout/workbenchControl";

export interface ProjectResourcesReadyContext {
  isCurrent(): boolean;
}

export interface WorkbenchLayoutController {
  readonly projectResourcesReady: boolean;
  bind(api: LayoutModelBinding, windowLabel: string): void;
  unbind(api?: LayoutModelBinding): void;
  whenHydrated(): Promise<void>;
  flushBeforeWindowClose(): Promise<void>;
  beginLayoutReset(): number;
  completeLayoutReset(epoch: number): void;
  invalidateForProjectReplacement(): void;
  markProjectResourcesReady(
    callback: (context: ProjectResourcesReadyContext) => void | Promise<void>,
  ): void;
}

type LayoutReader = (key: string) => string | null | Promise<string | null>;

type ControllerDependencies = {
  readonly layoutRead?: WorkbenchLayoutRead;
  readonly layoutControl?: WorkbenchLayoutControl;
  readonly internal?: WorkbenchLayoutInternal;
  readonly logsRead?: LogsLayoutRead;
  readonly logsControl?: LogsLayoutControl;
  readonly storage?: WorkbenchLayoutStorage;
  readonly read?: LayoutReader;
  readonly debounceMs?: number;
};

type BoundRoot = {
  readonly api: LayoutModelBinding;
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
  writeSuspensionDepth: number;
};

type PendingResourcesReady = {
  readonly requestId: number;
  readonly projectGeneration: number;
  readonly restoreEpoch: number;
  readonly bindingGeneration: number;
  readonly callback: (context: ProjectResourcesReadyContext) => void | Promise<void>;
};

const DEFAULT_PERSISTENCE_DEBOUNCE_MS = 250;

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
    writeSuspensionDepth: 0,
  };
}

function rootIsEmpty(api: LayoutModelBinding): boolean {
  return modelTabs(api.getModel()).length === 0;
}

export function createWorkbenchLayoutController(
  dependencies: ControllerDependencies = {},
): WorkbenchLayoutController {
  const hasInjectedRuntime =
    dependencies.layoutRead !== undefined ||
    dependencies.layoutControl !== undefined ||
    dependencies.internal !== undefined;
  if (
    hasInjectedRuntime &&
    (dependencies.layoutRead === undefined ||
      dependencies.layoutControl === undefined ||
      dependencies.internal === undefined)
  ) {
    throw new Error("read, control, and internal must be injected together");
  }

  const isolated = hasInjectedRuntime ? undefined : createWorkbenchLayoutRuntime();
  const read = dependencies.layoutRead ?? isolated!.read;
  const control = dependencies.layoutControl ?? isolated!.control;
  const internal = dependencies.internal ?? isolated!.internal;
  const hasInjectedLogs =
    dependencies.logsRead !== undefined || dependencies.logsControl !== undefined;
  if (
    hasInjectedLogs &&
    (dependencies.logsRead === undefined || dependencies.logsControl === undefined)
  ) {
    throw new Error("logsRead and logsControl must be injected together");
  }
  const isolatedLogs = hasInjectedLogs ? undefined : createLogsLayoutRuntime();
  const logsRead =
    dependencies.logsRead ??
    ({
      subscribe: isolatedLogs!.subscribe,
      getLatestSnapshot: isolatedLogs!.getLatestSnapshot,
    } satisfies LogsLayoutRead);
  const logsControl =
    dependencies.logsControl ??
    ({
      beginRestore: isolatedLogs!.beginRestore,
      stageRestore: isolatedLogs!.stageRestore,
      captureBoundSnapshot: isolatedLogs!.captureBoundSnapshot,
      resetToDefault: isolatedLogs!.resetToDefault,
    } satisfies LogsLayoutControl);
  const storage = dependencies.storage ?? workbenchLayoutStorage;
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

  const writePayload = (currentBound: BoundRoot, root: IJsonModel): void => {
    const payload = createPersistedWorkbenchLayout(root, logsRead.getLatestSnapshot());
    storage.setItem(currentBound.key, JSON.stringify(payload));
  };

  const writePersistedLayout = async (cycle: HydrationCycle, request: number): Promise<void> => {
    if (
      cycle.writeSuspensionDepth > 0 ||
      persistenceCycle !== cycle ||
      request !== persistenceRequest ||
      !isSuccessfullyHydrated(cycle)
    )
      return;

    const currentBound = bound;
    if (!currentBound) return;
    const root = await control.serialize();
    if (
      cycle.writeSuspensionDepth > 0 ||
      persistenceCycle !== cycle ||
      request !== persistenceRequest ||
      !isSuccessfullyHydrated(cycle) ||
      bound !== currentBound
    )
      return;

    writePayload(currentBound, root);
  };

  const schedulePersistence = (cycle: HydrationCycle): void => {
    if (
      cycle.writeSuspensionDepth > 0 ||
      persistenceCycle !== cycle ||
      !isSuccessfullyHydrated(cycle)
    )
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
    persistenceDisposers = [read.subscribePersistence(schedule), logsRead.subscribe(schedule)];
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
    if (!isCurrentCycle(cycle)) return;
    try {
      internal.installHydrationLayout(cycle.internalHydrationEpoch, ensureRestoredRootPanels);
    } catch {
      // A restored root remains usable if permanent-sidebar enforcement fails.
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
    layout: IJsonModel,
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

    const parsed = parseStoredWorkbenchLayout(raw);
    const logsLayout = parsed?.logs.status === "valid" ? parsed.logs.value : DEFAULT_LOGS_LAYOUT;
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
      currentBound.api.replace(parsed.root.value);
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
      const previousGeneration = bindingGeneration;
      if (bound) controller.unbind(bound.api);
      // Final capture may synchronously bind a successor, including the same API.
      if (bound || bindingGeneration !== previousGeneration) return;
      const generation = ++bindingGeneration;
      currentStorageKey = key;

      pausePersistence();
      const logsRestoreEpoch = logsControl.beginRestore();
      const internalHydrationEpoch = internal.beginHydration();
      if (bindingGeneration !== generation || bound) return;
      const nextBound: BoundRoot = {
        api,
        key,
        generation,
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

      if (cycle) cycle.writeSuspensionDepth += 1;
      invalidateScheduledWrites();
      try {
        if (canPersist) {
          try {
            logsControl.captureBoundSnapshot();
            if (isSuccessfullyHydrated(cycle) && bound === currentBound) {
              const root = currentBound.api.getModel().toJson();
              if (isSuccessfullyHydrated(cycle) && bound === currentBound) {
                writePayload(currentBound, root);
              }
            }
          } catch {
            // Unbind must still release the live FlexLayout API.
          }
        }
      } finally {
        if (cycle) cycle.writeSuspensionDepth -= 1;
        if (bound === currentBound) {
          pausePersistence();
          settleInvalidatedCycle();
          restoreEpoch += 1;
          hydratedEpoch = undefined;
          currentCycle = undefined;
          logsControl.beginRestore();
          resourcesReady = false;
          bound = undefined;
          internal.unbind(currentBound.api);
        }
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

      cycle.writeSuspensionDepth += 1;
      invalidateScheduledWrites();
      try {
        await internal.whenIdle();
        if (!isSuccessfullyHydrated(cycle) || bound !== currentBound) return;
        logsControl.captureBoundSnapshot();
        if (!isSuccessfullyHydrated(cycle) || bound !== currentBound) return;
        const root = currentBound.api.getModel().toJson();
        if (!isSuccessfullyHydrated(cycle) || bound !== currentBound) return;
        writePayload(currentBound, root);
      } finally {
        if (persistenceCycle === cycle) invalidateScheduledWrites();
        cycle.writeSuspensionDepth -= 1;
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
  layoutRead: workbenchLayoutRead,
  layoutControl: workbenchLayoutControl,
  internal: workbenchLayoutInternal,
  logsRead: logsLayoutRead,
  logsControl: logsLayoutControl,
  storage: workbenchLayoutStorage,
});
