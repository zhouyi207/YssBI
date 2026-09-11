import type {
  DockviewApi,
  DockviewGroupPanel,
  DockviewGroupPanelApi,
  IDockviewPanel,
} from "dockview-react";

import {
  canMoveWorkbenchPanel,
  canRemoveWorkbenchPanel,
  canSplitWorkbenchPanel,
  vetoInvalidWorkbenchActivityDrop,
} from "./workbenchActivityGroup";
import { WORKBENCH_HOME_EDGE } from "./workbenchDockviewDefaults";
import { componentForWorkbenchMetadata } from "./workbenchPanelModel";
import type {
  WorkbenchDockviewReadContract,
  WorkbenchDockviewControlContract,
  WorkbenchEdgePosition,
  WorkbenchEditorPanelInfo,
  WorkbenchPanelCommitToken,
} from "./workbenchTypes";
import { WorkbenchLayoutError } from "./workbenchTypes";
import { resultReferenceKey } from "@/shared/types/domain/result";

import {
  type Disposable,
  EDGE_POSITIONS,
  isRecord,
  readMetadata,
  metadataEqual,
  readLocation,
  panelInfo,
  isEditorPanelInfo,
  listPanelInfo,
  listGroupInfo,
  readEdgeState,
  throwAsLayoutError,
  requireValidMetadata,
  updatePanelMetadata,
  setGroupSize,
  validateEdgeSize,
  revealPanel,
  ensureCentralGroupLive,
  requireGridGroup,
  requireGroup,
  ensureHomeEdgeLive,
  configureEdgeLive,
  createPanelLive,
  remapLiveResources,
} from "./workbenchDockviewOperations";
import type { WorkbenchDockviewTransaction, WorkbenchLayoutTransaction } from "./workbenchTypes";
import { PendingWorkbenchTransaction } from "./workbenchDockviewTransaction";

export interface WorkbenchDockviewInternal {
  bind(api: DockviewApi): void;
  unbind(api?: DockviewApi): void;
  beginHydration(): number;
  completeHydration(epoch?: number): void;
  invalidateHydration(): void;
  invalidatePendingOperations(): void;
  whenIdle(): Promise<void>;
  commitRemove(
    expected: readonly WorkbenchPanelCommitToken[],
    authorize?: () => boolean,
  ): Promise<"committed" | "stale">;
  installHydrationLayout<T>(
    epoch: number,
    operation: (transaction: WorkbenchLayoutTransaction) => T,
  ): T;
  runLayoutTransaction<T>(operation: (transaction: WorkbenchLayoutTransaction) => T): Promise<T>;
  runPublicationTransaction<T>(
    operation: (transaction: WorkbenchDockviewTransaction) => T | Promise<T>,
  ): Promise<T>;
}

function isPromiseLike(value: unknown): value is PromiseLike<unknown> {
  return isRecord(value) && typeof value.then === "function";
}

export function createWorkbenchDockviewRuntime(): {
  readonly read: WorkbenchDockviewReadContract;
  readonly control: WorkbenchDockviewControlContract;
  readonly internal: WorkbenchDockviewInternal;
} {
  let api: DockviewApi | undefined;
  let bindingGeneration = 0;
  let operationGeneration = 0;
  let hydrated = false;
  let hydrationEpoch = 0;
  let revision = 0;
  let snapshot: Readonly<{ revision: number; ready: boolean; hydrated: boolean }> = Object.freeze({
    revision,
    ready: false,
    hydrated: false,
  });
  let draining = false;
  let listenerDeferralDepth = 0;
  let deferredNotification = false;
  const listeners = new Set<() => void>();
  const hydrationWaiters = new Set<(result: { readonly status: "hydrated" | "unbound" }) => void>();
  const idleWaiters = new Set<() => void>();
  const rootDisposables: Disposable[] = [];
  const edgeDisposables = new Map<
    WorkbenchEdgePosition,
    Disposable & { readonly edge: DockviewGroupPanelApi }
  >();
  const panelDisposables = new Map<string, Disposable & { readonly panel: IDockviewPanel }>();
  type PendingOperation = {
    readonly operationGeneration: number;
    readonly bindingGeneration?: number;
    run(boundApi: DockviewApi): Promise<void>;
    reject(error: unknown): void;
  };
  const queue: PendingOperation[] = [];

  const notifyListeners = (): void => {
    // Callbacks may subscribe or unsubscribe; publish to the captured observers once.
    const currentListeners = Array.from(listeners);
    for (const listener of currentListeners) {
      try {
        listener();
      } catch {
        // An observer cannot interrupt Dockview's authoritative mutation stream.
      }
    }
  };

  const publish = (): void => {
    revision += 1;
    snapshot = Object.freeze({ revision, ready: api !== undefined, hydrated });
    if (listenerDeferralDepth > 0) {
      deferredNotification = true;
      return;
    }
    notifyListeners();
  };

  const applyBufferedCommands = (hasCommands: boolean, operation: () => void): void => {
    const startingRevision = revision;
    let completed = false;
    listenerDeferralDepth += 1;
    try {
      operation();
      completed = true;
    } finally {
      listenerDeferralDepth -= 1;
      if (listenerDeferralDepth === 0) {
        if (completed && hasCommands && revision === startingRevision) {
          revision += 1;
          snapshot = Object.freeze({ revision, ready: api !== undefined, hydrated });
          deferredNotification = true;
        }
        if (deferredNotification) {
          deferredNotification = false;
          notifyListeners();
        }
      }
    }
  };

  const settleIdle = (): void => {
    if (draining || queue.length > 0) return;
    for (const resolve of idleWaiters) resolve();
    idleWaiters.clear();
  };

  const staleBindingError = (): WorkbenchLayoutError =>
    new WorkbenchLayoutError("dockview_not_ready", { reason: "stale_binding" });

  const rejectQueuedOperations = (predicate: (operation: PendingOperation) => boolean): void => {
    const retained: PendingOperation[] = [];
    for (const pending of queue) {
      if (predicate(pending)) pending.reject(staleBindingError());
      else retained.push(pending);
    }
    queue.splice(0, queue.length, ...retained);
    settleIdle();
  };

  const rebindEdgeListeners = (): void => {
    for (const position of EDGE_POSITIONS) {
      const edge = api?.getEdgeGroup(position);
      const existing = edgeDisposables.get(position);
      if (existing?.edge === edge) continue;
      existing?.dispose();
      edgeDisposables.delete(position);
      if (!edge) continue;
      const disposable = edge.onDidCollapsedChange(() => publish());
      edgeDisposables.set(position, { edge, dispose: () => disposable.dispose() });
    }
  };

  const disposeSubscriptions = (): void => {
    rootDisposables.splice(0).forEach((disposable) => disposable.dispose());
    for (const disposable of edgeDisposables.values()) disposable.dispose();
    edgeDisposables.clear();
    for (const disposable of panelDisposables.values()) disposable.dispose();
    panelDisposables.clear();
  };

  const rebindPanelListeners = (): void => {
    const panels = new Set(api?.panels);
    for (const [id, disposable] of panelDisposables) {
      if (panels.has(disposable.panel)) continue;
      disposable.dispose();
      panelDisposables.delete(id);
    }

    for (const panel of panels) {
      if (panelDisposables.has(panel.id)) continue;
      const disposables: Disposable[] = [];
      if (typeof panel.api.onDidVisibilityChange === "function") {
        disposables.push(panel.api.onDidVisibilityChange(() => publish()));
      }
      if (typeof panel.api.onDidGroupChange === "function") {
        disposables.push(panel.api.onDidGroupChange(() => publish()));
      }
      if (disposables.length > 0) {
        panelDisposables.set(panel.id, {
          panel,
          dispose: () => disposables.forEach((disposable) => disposable.dispose()),
        });
      }
    }
  };

  const drain = async (): Promise<void> => {
    if (draining || !api || !hydrated) return;
    draining = true;
    try {
      while (api && hydrated && queue.length > 0) {
        const next = queue.shift();
        if (next) await next.run(api);
      }
    } finally {
      draining = false;
      settleIdle();
      if (api && hydrated && queue.length > 0) void drain();
    }
  };

  type MutationContext = Readonly<{
    bindingGeneration: number;
    operationGeneration: number;
    hydrationEpoch: number;
  }>;

  const captureMutationContext = (): MutationContext => ({
    bindingGeneration,
    operationGeneration,
    hydrationEpoch,
  });

  const assertBindingContext = (boundApi: DockviewApi, expected: MutationContext): void => {
    if (
      api !== boundApi ||
      bindingGeneration !== expected.bindingGeneration ||
      operationGeneration !== expected.operationGeneration
    ) {
      throw staleBindingError();
    }
  };

  const assertMutationContext = (boundApi: DockviewApi, expected: MutationContext): void => {
    assertBindingContext(boundApi, expected);
    if (!hydrated || hydrationEpoch !== expected.hydrationEpoch) {
      throw new WorkbenchLayoutError("dockview_not_ready", { reason: "stale_hydration" });
    }
  };

  const assertHydrationLayoutContext = (
    boundApi: DockviewApi,
    expected: MutationContext,
    epoch: number,
  ): void => {
    assertBindingContext(boundApi, expected);
    if (hydrated || hydrationEpoch !== epoch || expected.hydrationEpoch !== epoch) {
      throw new WorkbenchLayoutError("dockview_not_ready", { reason: "stale_hydration" });
    }
  };

  const enqueue = <T>(operation: (boundApi: DockviewApi) => T | Promise<T>): Promise<T> => {
    const queuedOperationGeneration = operationGeneration;
    const queuedBindingGeneration = api === undefined ? undefined : bindingGeneration;
    return new Promise<T>((resolve, rejectPromise) => {
      let settled = false;
      const settleReject = (error: unknown): void => {
        if (settled) return;
        settled = true;
        rejectPromise(error);
      };
      queue.push({
        operationGeneration: queuedOperationGeneration,
        ...(queuedBindingGeneration === undefined
          ? {}
          : { bindingGeneration: queuedBindingGeneration }),
        reject: settleReject,
        async run(boundApi) {
          if (settled) return;
          try {
            if (
              operationGeneration !== queuedOperationGeneration ||
              (queuedBindingGeneration !== undefined &&
                (bindingGeneration !== queuedBindingGeneration || api !== boundApi))
            ) {
              throw staleBindingError();
            }
            const result = await operation(boundApi);
            if (settled) return;
            settled = true;
            resolve(result);
          } catch (error) {
            settleReject(error);
          }
        },
      });
      void drain();
    });
  };

  const runtime: WorkbenchDockviewReadContract & WorkbenchDockviewControlContract = {
    get isReady() {
      return api !== undefined;
    },
    get isHydrated() {
      return hydrated;
    },
    whenHydrated: () =>
      hydrated
        ? Promise.resolve({ status: "hydrated" as const })
        : new Promise((resolve) => hydrationWaiters.add(resolve)),
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    getSnapshot: () => snapshot,
    getPanel: (panelInstanceId) => panelInfo(api?.getPanel(panelInstanceId)),
    getActivePanel: () => panelInfo(api?.activePanel),
    getActiveEditorPanel: () => {
      const active = panelInfo(api?.activePanel);
      return active && isEditorPanelInfo(active) ? active : undefined;
    },
    getActiveEditorPanelInGroup: (groupId) => {
      const group = api?.getGroup(groupId);
      const active = panelInfo(group?.activePanel);
      return active && isEditorPanelInfo(active) ? active : undefined;
    },
    listPanels: () => (api ? listPanelInfo(api) : []),
    listGroups: () => (api ? listGroupInfo(api) : []),
    listGroupPanels: (groupId) => {
      const group = api?.getGroup(groupId);
      if (!group) return [];
      return group.panels.flatMap((panel) => {
        const info = panelInfo(panel);
        return info ? [info] : [];
      });
    },
    listEditorPanelsInGroup: (groupId) => {
      const group = api?.getGroup(groupId);
      if (!group) return [];
      return group.panels.flatMap((panel) => {
        const info = panelInfo(panel);
        return info && isEditorPanelInfo(info) ? [info] : [];
      });
    },
    findEditorPanelsByResource: (resourceRef) =>
      api
        ? listPanelInfo(api).filter(
            (panel): panel is WorkbenchEditorPanelInfo =>
              isEditorPanelInfo(panel) && panel.metadata.resourceRef === resourceRef,
          )
        : [],
    getEdgeState: (position) =>
      api
        ? readEdgeState(api, position)
        : { position, exists: false, visible: false, collapsed: false },
    ensureCentralGroup: () =>
      enqueue((boundApi) =>
        throwAsLayoutError("panel_open_failed", {}, () => ensureCentralGroupLive(boundApi)),
      ),
    openEditor: (request) =>
      enqueue((boundApi) =>
        throwAsLayoutError(
          "panel_open_failed",
          request.targetGroupId ? { groupId: request.targetGroupId } : {},
          () => {
            const metadata = requireValidMetadata({
              role: "editor",
              resourceRef: request.resourceRef,
              resourceKind: request.resourceKind,
              ...(request.sticky === undefined ? {} : { sticky: request.sticky }),
            });
            if (request.mode === "reuse-resource") {
              const existing = boundApi.panels.find((candidate) => {
                const candidateMetadata = readMetadata(candidate);
                return (
                  candidateMetadata?.role === "editor" &&
                  candidateMetadata.resourceRef === request.resourceRef
                );
              });
              if (existing) {
                const existingMetadata = readMetadata(existing);
                const requestedComponent = componentForWorkbenchMetadata(metadata);
                if (
                  existingMetadata?.role !== "editor" ||
                  existingMetadata.resourceKind !== request.resourceKind ||
                  existing.api.component !== requestedComponent
                ) {
                  throw new WorkbenchLayoutError("panel_open_failed", {
                    panelInstanceId: existing.id,
                  });
                }
                updatePanelMetadata(existing, metadata);
                if (existing.title !== request.title) existing.api.setTitle(request.title);
                existing.api.setPinned(true);
                revealPanel(boundApi, existing);
                const info = panelInfo(existing);
                if (info) return info;
              }
            }
            const groupId = request.targetGroupId
              ? requireGridGroup(boundApi, request.targetGroupId).id
              : ensureCentralGroupLive(boundApi);
            const panel = createPanelLive(
              boundApi,
              metadata,
              request.title,
              groupId,
              request.index,
            );
            panel.api.setPinned(true);
            const info = panelInfo(panel);
            if (!info) throw new WorkbenchLayoutError("invalid_panel_metadata");
            return info;
          },
        ),
      ),
    ensureView: (request) =>
      enqueue((boundApi) =>
        throwAsLayoutError("panel_open_failed", { viewId: request.viewId }, () => {
          const existing = boundApi.panels.find((candidate) => {
            const metadata = readMetadata(candidate);
            return metadata?.role === "view" && metadata.viewId === request.viewId;
          });
          if (existing) {
            if (existing.title !== request.title) existing.api.setTitle(request.title);
            revealPanel(boundApi, existing);
            const info = panelInfo(existing);
            if (info) return info;
          }
          const position = WORKBENCH_HOME_EDGE[request.viewId];
          const group = ensureHomeEdgeLive(boundApi, position);
          const panel = createPanelLive(
            boundApi,
            { role: "view", viewId: request.viewId },
            request.title,
            group.id,
          );
          revealPanel(boundApi, panel);
          rebindEdgeListeners();
          const info = panelInfo(panel);
          if (!info) throw new WorkbenchLayoutError("invalid_panel_metadata");
          return info;
        }),
      ),
    upsertResult: (request) =>
      enqueue((boundApi) =>
        throwAsLayoutError(
          "panel_open_failed",
          { reference: resultReferenceKey(request.reference) },
          () => {
            const metadata = requireValidMetadata({ role: "result", ...request });
            const existing = boundApi.panels.find((candidate) => {
              const candidateMetadata = readMetadata(candidate);
              return (
                candidateMetadata?.role === "result" &&
                resultReferenceKey(candidateMetadata.reference) ===
                  resultReferenceKey(request.reference)
              );
            });
            if (existing) {
              revealPanel(boundApi, existing);
              const info = panelInfo(existing);
              if (info) return info;
            }
            const position = WORKBENCH_HOME_EDGE.result;
            const group = ensureHomeEdgeLive(boundApi, position);
            const panel = createPanelLive(boundApi, metadata, request.title, group.id);
            revealPanel(boundApi, panel);
            rebindEdgeListeners();
            const info = panelInfo(panel);
            if (!info) throw new WorkbenchLayoutError("invalid_panel_metadata");
            return info;
          },
        ),
      ),
    activate: (panelInstanceId) =>
      enqueue((boundApi) =>
        throwAsLayoutError("panel_open_failed", { panelInstanceId }, () => {
          const panel = boundApi.getPanel(panelInstanceId);
          if (!panel || !readMetadata(panel)) return false;
          panel.api.setActive();
          return true;
        }),
      ),
    reveal: (panelInstanceId) =>
      enqueue((boundApi) =>
        throwAsLayoutError("panel_open_failed", { panelInstanceId }, () => {
          const panel = boundApi.getPanel(panelInstanceId);
          if (!panel || !readMetadata(panel)) return false;
          revealPanel(boundApi, panel);
          return true;
        }),
      ),
    move: (request) =>
      enqueue((boundApi) =>
        throwAsLayoutError("group_not_found", { groupId: request.groupId }, () => {
          const panel = boundApi.getPanel(request.panelInstanceId);
          const metadata = panel ? readMetadata(panel) : undefined;
          if (!panel || !metadata) return false;
          const target = requireGroup(boundApi, request.groupId);
          const targetLocation = readLocation(target);
          if (!targetLocation) return false;
          const targetPosition =
            targetLocation.type === "edge" ? targetLocation.position : targetLocation.type;
          if (!canMoveWorkbenchPanel(metadata, target.id, targetPosition)) return false;
          const source = panel.group;
          const currentIndex = source.panels.indexOf(panel);
          const maximumIndex =
            source.id === target.id ? Math.max(0, target.panels.length - 1) : target.panels.length;
          const effectiveIndex =
            request.index === undefined
              ? source.id === target.id
                ? currentIndex
                : undefined
              : Math.min(Math.max(request.index, 0), maximumIndex);
          if (
            source.id === target.id &&
            (effectiveIndex === undefined || effectiveIndex === currentIndex)
          ) {
            if (request.activate !== false) panel.api.setActive();
            return true;
          }
          panel.api.moveTo({
            group: target as DockviewGroupPanel,
            ...(effectiveIndex === undefined ? {} : { index: effectiveIndex }),
            skipSetActive: request.activate === false,
          });
          return true;
        }),
      ),
    split: (request) =>
      enqueue((boundApi) =>
        throwAsLayoutError("group_not_found", { groupId: request.referenceGroupId }, () => {
          const panel = boundApi.getPanel(request.panelInstanceId);
          const metadata = panel ? readMetadata(panel) : undefined;
          if (!panel || !metadata) return false;
          const reference = requireGroup(boundApi, request.referenceGroupId);
          if (!canSplitWorkbenchPanel(metadata, reference.id)) return false;
          panel.api.moveTo({
            group: reference as DockviewGroupPanel,
            position: request.direction,
            skipSetActive: request.activate === false,
          });
          return true;
        }),
      ),
    configureEdge: (request) =>
      enqueue((boundApi) =>
        throwAsLayoutError("layout_restore_failed", { position: request.position }, () => {
          const state = configureEdgeLive(boundApi, request);
          rebindEdgeListeners();
          return state;
        }),
      ),
    setEdgeCollapsed: (position, collapsed) =>
      enqueue((boundApi) =>
        throwAsLayoutError("layout_restore_failed", { position }, () => {
          const edge = boundApi.getEdgeGroup(position);
          if (!edge) return false;
          if (collapsed) {
            edge.collapse();
            if (position === "bottom") boundApi.setEdgeGroupVisible(position, false);
          } else {
            boundApi.setEdgeGroupVisible(position, true);
            edge.expand();
          }
          return true;
        }),
      ),
    setEdgeSize: (position, size) =>
      enqueue((boundApi) =>
        throwAsLayoutError("layout_restore_failed", { position }, () => {
          validateEdgeSize(position, size);
          const edge = boundApi.getEdgeGroup(position);
          if (!edge) return false;
          setGroupSize(edge, position, size);
          return true;
        }),
      ),
    remapResource: (from, to) =>
      enqueue((boundApi) =>
        throwAsLayoutError("panel_open_failed", {}, () => remapLiveResources(boundApi, from, to)),
      ),
    serialize: () =>
      enqueue((boundApi) =>
        throwAsLayoutError("layout_restore_failed", {}, () => structuredClone(boundApi.toJSON())),
      ),
  };

  const internal: WorkbenchDockviewInternal = {
    bind(boundApi) {
      if (api === boundApi) {
        rebindEdgeListeners();
        void drain();
        return;
      }
      disposeSubscriptions();
      bindingGeneration += 1;
      api = boundApi;
      rootDisposables.push(
        boundApi.onDidLayoutChange(() => {
          rebindEdgeListeners();
          rebindPanelListeners();
          publish();
        }),
        boundApi.onDidLayoutFromJSON(() => {
          rebindEdgeListeners();
          rebindPanelListeners();
          publish();
        }),
        boundApi.onWillShowOverlay((event) => {
          vetoInvalidWorkbenchActivityDrop(event);
        }),
        boundApi.onWillDrop((event) => {
          vetoInvalidWorkbenchActivityDrop(event);
        }),
        boundApi.onDidActivePanelChange(() => publish()),
        boundApi.onDidActiveGroupChange(() => publish()),
      );
      rebindEdgeListeners();
      rebindPanelListeners();
      publish();
      void drain();
    },
    unbind(boundApi) {
      if (boundApi && boundApi !== api) return;
      const invalidatedBindingGeneration = bindingGeneration;
      disposeSubscriptions();
      bindingGeneration += 1;
      api = undefined;
      for (const resolve of hydrationWaiters) resolve({ status: "unbound" });
      hydrationWaiters.clear();
      rejectQueuedOperations(
        (pending) => pending.bindingGeneration === invalidatedBindingGeneration,
      );
      publish();
    },
    beginHydration() {
      hydrationEpoch += 1;
      if (hydrated) {
        hydrated = false;
        publish();
      }
      return hydrationEpoch;
    },
    completeHydration(epoch) {
      if (epoch !== undefined && epoch !== hydrationEpoch) return;
      if (!hydrated) {
        hydrated = true;
        for (const resolve of hydrationWaiters) resolve({ status: "hydrated" });
        hydrationWaiters.clear();
        publish();
      }
      void drain();
    },
    invalidateHydration() {
      hydrationEpoch += 1;
      if (hydrated) {
        hydrated = false;
        publish();
      }
    },
    invalidatePendingOperations() {
      operationGeneration += 1;
      rejectQueuedOperations(() => true);
    },
    whenIdle: () =>
      !draining && queue.length === 0
        ? Promise.resolve()
        : new Promise<void>((resolve) => idleWaiters.add(resolve)),
    commitRemove: (expected, authorize) =>
      enqueue((boundApi) => {
        if (authorize) {
          try {
            if (!authorize()) return "stale" as const;
          } catch {
            return "stale" as const;
          }
        }
        const panels: IDockviewPanel[] = [];
        const seen = new Set<string>();
        for (const token of expected) {
          const panel = throwAsLayoutError(
            "layout_restore_failed",
            { panelInstanceId: token.panelInstanceId },
            () => boundApi.getPanel(token.panelInstanceId),
          );
          const metadata = panel
            ? throwAsLayoutError(
                "layout_restore_failed",
                { panelInstanceId: token.panelInstanceId },
                () => readMetadata(panel),
              )
            : undefined;
          if (
            !panel ||
            !metadata ||
            panel.group.id !== token.groupId ||
            !metadataEqual(metadata, token.metadata) ||
            !canRemoveWorkbenchPanel(metadata)
          ) {
            return "stale" as const;
          }
          if (!seen.has(token.panelInstanceId)) {
            seen.add(token.panelInstanceId);
            panels.push(panel);
          }
        }
        panels.forEach((panel) =>
          throwAsLayoutError("layout_restore_failed", { panelInstanceId: panel.id }, () =>
            panel.api.close(),
          ),
        );
        return "committed" as const;
      }),
    installHydrationLayout: (epoch, operation) => {
      const boundApi = api;
      if (!boundApi) throw staleBindingError();
      const context = captureMutationContext();
      assertHydrationLayoutContext(boundApi, context, epoch);
      const shadow = throwAsLayoutError(
        "layout_restore_failed",
        {},
        () => new PendingWorkbenchTransaction(boundApi),
      );
      const result = operation(shadow.layout);
      if (isPromiseLike(result)) {
        throw new WorkbenchLayoutError("layout_restore_failed", {
          reason: "async_layout_transaction",
        });
      }
      assertHydrationLayoutContext(boundApi, context, epoch);
      throwAsLayoutError("layout_restore_failed", {}, () => shadow.validate(boundApi));
      throwAsLayoutError("layout_restore_failed", {}, () =>
        applyBufferedCommands(shadow.hasBufferedCommands(), () => {
          shadow.apply(boundApi);
          rebindEdgeListeners();
        }),
      );
      return result;
    },
    runLayoutTransaction: (operation) =>
      enqueue((boundApi) => {
        const context = captureMutationContext();
        const shadow = throwAsLayoutError(
          "layout_restore_failed",
          {},
          () => new PendingWorkbenchTransaction(boundApi),
        );
        const result = operation(shadow.layout);
        if (isPromiseLike(result)) {
          throw new WorkbenchLayoutError("layout_restore_failed", {
            reason: "async_layout_transaction",
          });
        }
        assertMutationContext(boundApi, context);
        throwAsLayoutError("layout_restore_failed", {}, () => shadow.validate(boundApi));
        throwAsLayoutError("layout_restore_failed", {}, () =>
          applyBufferedCommands(shadow.hasBufferedCommands(), () => {
            shadow.apply(boundApi);
            rebindEdgeListeners();
          }),
        );
        return result;
      }),
    runPublicationTransaction: (operation) =>
      enqueue(async (boundApi) => {
        const context = captureMutationContext();
        const shadow = throwAsLayoutError(
          "layout_restore_failed",
          {},
          () => new PendingWorkbenchTransaction(boundApi),
        );
        const result = await operation(shadow.publication);
        assertMutationContext(boundApi, context);
        throwAsLayoutError("layout_restore_failed", {}, () => shadow.validate(boundApi));
        throwAsLayoutError("layout_restore_failed", {}, () =>
          applyBufferedCommands(shadow.hasBufferedCommands(), () => {
            shadow.apply(boundApi);
            rebindEdgeListeners();
          }),
        );
        return result;
      }),
  };

  return { read: runtime, control: runtime, internal };
}

export const workbenchDockviewRuntime = createWorkbenchDockviewRuntime();

export const workbenchDockviewInternal = workbenchDockviewRuntime.internal;
