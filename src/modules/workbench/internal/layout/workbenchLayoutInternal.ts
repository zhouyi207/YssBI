import { Actions, type Action } from "flexlayout-react";
import { LayoutModelBinding } from "./layoutModelBinding";
import { WorkbenchModelOperations, metadataEqual } from "./workbenchLayoutOperations";
import { PendingWorkbenchTransaction } from "./workbenchLayoutTransaction";
import { canRemoveWorkbenchPanel } from "./workbenchActivityGroup";
import {
  WorkbenchLayoutError,
  type WorkbenchLayoutReadContract,
  type WorkbenchLayoutControlContract,
  type WorkbenchEditorPanelInfo,
  type WorkbenchPanelCommitToken,
  type WorkbenchLayoutTransaction,
  type WorkbenchPublicationTransaction,
} from "./workbenchTypes";

export interface WorkbenchLayoutInternal {
  bind(binding: LayoutModelBinding): void;
  unbind(binding?: LayoutModelBinding): void;
  beginHydration(): number;
  completeHydration(epoch?: number): void;
  invalidateHydration(): void;
  invalidatePendingOperations(): void;
  whenIdle(): Promise<void>;
  dispatchAction(action: Action): void;
  activatePanel(id: string): void;
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
    operation: (transaction: WorkbenchPublicationTransaction) => T | Promise<T>,
  ): Promise<T>;
}

export function createWorkbenchLayoutRuntime(): {
  readonly read: WorkbenchLayoutReadContract;
  readonly control: WorkbenchLayoutControlContract;
  readonly internal: WorkbenchLayoutInternal;
} {
  let binding: LayoutModelBinding | undefined;
  let unsubscribe: (() => void) | undefined;
  let generation = 0;
  let hydrationEpoch = 0;
  let hydrated = false;
  let revision = 0;
  let snapshot = { revision, ready: false, hydrated };
  let draining = false;
  const listeners = new Set<() => void>();
  const hydrationWaiters = new Set<(result: { status: "hydrated" | "unbound" }) => void>();
  const idleWaiters = new Set<() => void>();
  type Pending = { run: () => Promise<void>; reject: (error: unknown) => void };
  const queue: Pending[] = [];
  const stale = () => new WorkbenchLayoutError("layout_not_ready", { reason: "stale_binding" });
  const publish = () => {
    snapshot = { revision: ++revision, ready: Boolean(binding), hydrated };
    for (const listener of [...listeners]) {
      try {
        listener();
      } catch {
        /* Isolate subscribers from layout commits. */
      }
    }
  };
  const ops = () => (binding ? new WorkbenchModelOperations(binding.getModel()) : undefined);
  const settleIdle = () => {
    if (!draining && !queue.length) {
      for (const resolve of idleWaiters) resolve();
      idleWaiters.clear();
    }
  };
  const invalidate = () => {
    generation++;
    for (const pending of queue.splice(0)) pending.reject(stale());
    settleIdle();
  };
  const drain = async () => {
    if (draining || !binding || !hydrated) return;
    draining = true;
    try {
      while (binding && hydrated && queue.length) await queue.shift()!.run();
    } finally {
      draining = false;
      settleIdle();
      if (binding && hydrated && queue.length) void drain();
    }
  };
  const enqueue = <T>(
    operation: (current: LayoutModelBinding, isCurrent: () => boolean) => T | Promise<T>,
  ): Promise<T> => {
    const expectedGeneration = generation;
    return new Promise<T>((resolve, reject) => {
      queue.push({
        reject,
        run: async () => {
          const current = binding;
          const isCurrent = () =>
            binding === current && generation === expectedGeneration && hydrated;
          if (!current || !isCurrent()) {
            reject(stale());
            return;
          }
          try {
            resolve(await operation(current, isCurrent));
          } catch (error) {
            reject(error);
          }
        },
      });
      void drain();
    });
  };
  const mutate = <T>(operation: (model: WorkbenchModelOperations) => T) =>
    enqueue((current) => operation(new WorkbenchModelOperations(current.getModel())));
  const transaction = <T>(
    current: LayoutModelBinding,
    operation: (tx: WorkbenchLayoutTransaction) => T,
  ): T => {
    const pending = new PendingWorkbenchTransaction(current);
    const result = operation(pending.operations);
    if (result && typeof (result as { then?: unknown }).then === "function")
      throw new WorkbenchLayoutError("layout_restore_failed", {
        reason: "async_layout_transaction",
      });
    pending.commit();
    return result;
  };

  const read: WorkbenchLayoutReadContract = {
    get isReady() {
      return Boolean(binding);
    },
    get isHydrated() {
      return hydrated;
    },
    whenHydrated: () =>
      hydrated
        ? Promise.resolve({ status: "hydrated" })
        : new Promise((resolve) => hydrationWaiters.add(resolve)),
    subscribe: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    getSnapshot: () => snapshot,
    getPanel: (id) => ops()?.getPanel(id),
    getActivePanel: () => ops()?.getActivePanel(),
    getActiveEditorPanel: () => {
      const panel = ops()?.getActivePanel();
      return panel?.metadata.role === "editor" ? (panel as WorkbenchEditorPanelInfo) : undefined;
    },
    getActiveEditorPanelInGroup: (id) => {
      const model = ops();
      const group = model?.listGroups().find((entry) => entry.groupId === id);
      const panel = group?.activePanelInstanceId
        ? model?.getPanel(group.activePanelInstanceId)
        : undefined;
      return panel?.metadata.role === "editor" ? (panel as WorkbenchEditorPanelInfo) : undefined;
    },
    listPanels: () => ops()?.listPanels() ?? [],
    listGroups: () => ops()?.listGroups() ?? [],
    listGroupPanels: (id) => ops()?.listGroupPanels(id) ?? [],
    listEditorPanelsInGroup: (id) =>
      (ops()?.listGroupPanels(id) ?? []).filter(
        (panel): panel is WorkbenchEditorPanelInfo => panel.metadata.role === "editor",
      ),
    findEditorPanelsByResource: (resourceRef) =>
      (ops()?.listPanels() ?? []).filter(
        (panel): panel is WorkbenchEditorPanelInfo =>
          panel.metadata.role === "editor" && panel.metadata.resourceRef === resourceRef,
      ),
    getEdgeState: (position) =>
      ops()?.getEdgeState(position) ?? { position, exists: false, visible: false, collapsed: true },
  };
  const control: WorkbenchLayoutControlContract = {
    ensureCentralGroup: () => mutate((model) => model.ensureCentralGroup()),
    openEditor: (request) => mutate((model) => model.openEditor(request)),
    ensureView: (request) => mutate((model) => model.ensureView(request)),
    upsertResult: (request) => mutate((model) => model.upsertResult(request)),
    activate: (id) => mutate((model) => model.activate(id)),
    reveal: (id) => mutate((model) => model.reveal(id)),
    move: (request) => mutate((model) => model.move(request)),
    split: (request) => mutate((model) => model.split(request)),
    configureEdge: (request) => mutate((model) => model.configureEdge(request)),
    setEdgeCollapsed: (position, collapsed) =>
      mutate((model) => model.setEdgeCollapsed(position, collapsed)),
    setEdgeSize: (position, size) => mutate((model) => model.setEdgeSize(position, size)),
    remapResource: (from, to) => mutate((model) => model.remapResource(from, to)),
    serialize: () => enqueue((current) => structuredClone(current.getModel().toJson())),
  };
  const internal: WorkbenchLayoutInternal = {
    bind(next) {
      if (binding === next) return;
      if (binding) invalidate();
      unsubscribe?.();
      binding = next;
      unsubscribe = next.subscribe(publish);
      publish();
      void drain();
    },
    unbind(expected) {
      if (expected && binding !== expected) return;
      unsubscribe?.();
      unsubscribe = undefined;
      binding = undefined;
      hydrated = false;
      invalidate();
      for (const resolve of hydrationWaiters) resolve({ status: "unbound" });
      hydrationWaiters.clear();
      publish();
    },
    beginHydration() {
      hydrationEpoch++;
      hydrated = false;
      publish();
      return hydrationEpoch;
    },
    completeHydration(epoch) {
      if (epoch !== undefined && epoch !== hydrationEpoch) return;
      hydrated = true;
      publish();
      for (const resolve of hydrationWaiters) resolve({ status: "hydrated" });
      hydrationWaiters.clear();
      void drain();
    },
    invalidateHydration() {
      hydrationEpoch++;
      hydrated = false;
      publish();
    },
    invalidatePendingOperations: invalidate,
    whenIdle: () =>
      !draining && !queue.length
        ? Promise.resolve()
        : new Promise((resolve) => idleWaiters.add(resolve)),
    dispatchAction(action) {
      if (
        !binding ||
        !hydrated ||
        action.type === Actions.DELETE_TAB ||
        action.type === Actions.DELETE_TABSET
      )
        return;
      const model = binding.getModel();
      model.doAction(action);
    },
    activatePanel(id) {
      if (hydrated) ops()?.activate(id);
    },
    commitRemove: (expected, authorize) =>
      enqueue((current, isCurrent) => {
        try {
          if (!isCurrent() || (authorize && !authorize())) return "stale";
        } catch {
          return "stale";
        }
        const model = new WorkbenchModelOperations(current.getModel());
        for (const token of expected) {
          const panel = model.getPanel(token.panelInstanceId);
          if (
            !panel ||
            panel.groupId !== token.groupId ||
            !metadataEqual(panel.metadata, token.metadata) ||
            !canRemoveWorkbenchPanel(panel.metadata)
          )
            return "stale";
        }
        transaction(current, (tx) =>
          tx.removePanels(expected.map((token) => token.panelInstanceId)),
        );
        return "committed";
      }),
    installHydrationLayout(epoch, operation) {
      if (!binding || epoch !== hydrationEpoch) throw stale();
      return transaction(binding, operation);
    },
    runLayoutTransaction: (operation) => enqueue((current) => transaction(current, operation)),
    runPublicationTransaction: (operation) =>
      enqueue(async (current, isCurrent) => {
        const pending = new PendingWorkbenchTransaction(current);
        const result = await operation(pending.operations);
        if (!isCurrent()) throw stale();
        pending.commit();
        return result;
      }),
  };
  return { read, control, internal };
}
export const workbenchLayoutRuntime = createWorkbenchLayoutRuntime();
export const workbenchLayoutInternal = workbenchLayoutRuntime.internal;
