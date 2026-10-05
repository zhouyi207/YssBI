import { Actions, GroupAction, type Action } from "flexlayout-react";
import { LayoutModelBinding } from "./layoutModelBinding";
import { createWorkbenchLayoutProjection } from "./workbenchLayoutProjection";
import { WorkbenchModelOperations, metadataEqual } from "./workbenchLayoutOperations";
import { PendingWorkbenchTransaction } from "./workbenchLayoutTransaction";
import { canRemoveWorkbenchPanel } from "./workbenchActivityGroup";
import {
  WorkbenchLayoutError,
  type WorkbenchLayoutReadContract,
  type WorkbenchLayoutControlContract,
  type WorkbenchPanelCommitToken,
  type WorkbenchLayoutTransaction,
  type WorkbenchPublicationTransaction,
} from "./workbenchTypes";

export interface WorkbenchLayoutInternal {
  bind(binding: LayoutModelBinding): void;
  unbind(binding?: LayoutModelBinding): void;
  beginHydration(): number;
  completeHydration(epoch?: number): void;
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
  let draining = false;
  const projection = createWorkbenchLayoutProjection();
  const idleWaiters = new Set<() => void>();
  type Pending = { run: () => Promise<void>; reject: (error: unknown) => void };
  const queue: Pending[] = [];
  const stale = () => new WorkbenchLayoutError("layout_not_ready", { reason: "stale_binding" });
  const ops = () => (binding ? new WorkbenchModelOperations(binding.getModel()) : undefined);
  const publish = () => projection.publish(ops(), hydrated);
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
    ...projection.read,
    getMutationRevision: () =>
      `${generation}:${hydrationEpoch}:${hydrated}:${binding?.getSnapshot().revision ?? -1}`,
  };
  const control: WorkbenchLayoutControlContract = {
    ensureCentralGroup: () => mutate((model) => model.ensureCentralGroup()),
    openEditor: (request) => mutate((model) => model.openEditor(request)),
    openReference: (request) => mutate((model) => model.openReference(request)),
    openConversation: (request) => mutate((model) => model.openConversation(request)),
    updateConversationTitle: (request) => mutate((model) => model.updateConversationTitle(request)),
    ensureView: (request) => mutate((model) => model.ensureView(request)),
    upsertResult: (request) => mutate((model) => model.upsertResult(request)),
    replaceResult: (expected, request) => mutate((model) => model.replaceResult(expected, request)),
    activate: (id) => mutate((model) => model.activate(id)),
    reveal: (id) => mutate((model) => model.reveal(id)),
    move: (request) => mutate((model) => model.move(request)),
    split: (request) => mutate((model) => model.split(request)),
    configureEdge: (request) => mutate((model) => model.configureEdge(request)),
    floatPanel: (id) => mutate((model) => model.floatPanel(id)),
    floatGroup: (id) => mutate((model) => model.floatGroup(id)),
    dockFloat: (id) => mutate((model) => model.dockFloat(id)),
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
      void drain();
    },
    invalidatePendingOperations: invalidate,
    whenIdle: () =>
      !draining && !queue.length
        ? Promise.resolve()
        : new Promise((resolve) => idleWaiters.add(resolve)),
    dispatchAction(action) {
      const allowed = (candidate: Action): boolean => {
        if (candidate instanceof GroupAction) return candidate.actions.every(allowed);
        if (
          [
            Actions.DELETE_TAB,
            Actions.DELETE_TABSET,
            Actions.POPOUT_FLOAT,
            Actions.CREATE_SUBLAYOUT,
          ].includes(candidate.type)
        )
          return false;
        if ([Actions.POPOUT_TAB, Actions.POPOUT_TABSET].includes(candidate.type))
          return candidate.data.type === "float";
        return true;
      };
      if (!binding || !hydrated || !allowed(action)) return;
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
