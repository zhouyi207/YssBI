import type { IJsonModel } from "flexlayout-react";
import type { LayoutModelBinding } from "./layoutModelBinding";

import { DEFAULT_LOGS_LAYOUT } from "./logsLayoutModel";

export interface LogsLayoutRuntime {
  bind(api: LayoutModelBinding): void;
  unbind(api?: LayoutModelBinding): void;
  subscribe(listener: () => void): () => void;
  beginRestore(): number;
  stageRestore(epoch: number, layout: IJsonModel): "staged" | "applied" | "stale";
  captureBoundSnapshot(): void;
  getLatestSnapshot(): IJsonModel;
  resetToDefault(): void;
}

type BoundFlexLayout = {
  readonly api: LayoutModelBinding;
  readonly dispose: () => void;
};

type UnknownRecord = Record<string, unknown>;

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function comparableKeys(value: UnknownRecord): string[] {
  return Object.keys(value)
    .filter((key) => value[key] !== undefined)
    .sort();
}

function snapshotsEqual(left: unknown, right: unknown): boolean {
  if (Object.is(left, right)) return true;
  if (Array.isArray(left) || Array.isArray(right)) {
    return (
      Array.isArray(left) &&
      Array.isArray(right) &&
      left.length === right.length &&
      left.every((value, index) => snapshotsEqual(value, right[index]))
    );
  }
  if (!isRecord(left) || !isRecord(right)) return false;

  const leftKeys = comparableKeys(left);
  const rightKeys = comparableKeys(right);
  return (
    leftKeys.length === rightKeys.length &&
    leftKeys.every(
      (key, index) => key === rightKeys[index] && snapshotsEqual(left[key], right[key]),
    )
  );
}

function cloneLayout(layout: IJsonModel): IJsonModel {
  return structuredClone(layout);
}

export function createLogsLayoutRuntime(
  defaultLayout: IJsonModel = DEFAULT_LOGS_LAYOUT,
): LogsLayoutRuntime {
  const defaultSnapshot = cloneLayout(defaultLayout);
  const listeners = new Set<() => void>();
  let latestSnapshot = cloneLayout(defaultSnapshot);
  let pendingSnapshot: IJsonModel | undefined = cloneLayout(defaultSnapshot);
  let restoreEpoch = 0;
  let bound: BoundFlexLayout | undefined;

  const publishSnapshot = (layout: IJsonModel): boolean => {
    const next = cloneLayout(layout);
    if (snapshotsEqual(latestSnapshot, next)) return false;

    latestSnapshot = next;
    for (const listener of [...listeners]) {
      try {
        listener();
      } catch {
        // Observer failures must not interrupt layout lifecycle transitions.
      }
    }
    return true;
  };

  const capture = (api: LayoutModelBinding): void => {
    publishSnapshot(api.getModel().toJson());
  };

  const applyPending = (api: LayoutModelBinding): void => {
    const pending = pendingSnapshot;
    if (!pending) return;

    api.replace(cloneLayout(pending));
    if (bound?.api === api && pendingSnapshot === pending) {
      pendingSnapshot = undefined;
    }
  };

  const runtime: LogsLayoutRuntime = {
    bind(api) {
      if (bound?.api === api) return;
      if (bound) runtime.unbind(bound.api);

      const dispose = api.subscribe(() => {
        if (bound?.api === api) capture(api);
      });
      bound = { api, dispose };
      try {
        applyPending(api);
      } catch (error) {
        if (bound?.api === api) {
          bound = undefined;
          dispose();
        }
        throw error;
      }
    },

    unbind(api) {
      const current = bound;
      if (!current || (api !== undefined && api !== current.api)) return;

      let failure: { readonly value: unknown } | undefined;
      try {
        capture(current.api);
      } catch (error) {
        failure = { value: error };
      } finally {
        try {
          pendingSnapshot = cloneLayout(latestSnapshot);
        } catch (error) {
          failure ??= { value: error };
        }
        bound = undefined;
        try {
          current.dispose();
        } catch (error) {
          failure ??= { value: error };
        }
      }
      if (failure) throw failure.value;
    },

    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },

    beginRestore() {
      restoreEpoch += 1;
      return restoreEpoch;
    },

    stageRestore(epoch, layout) {
      if (epoch !== restoreEpoch) return "stale";

      pendingSnapshot = cloneLayout(layout);
      publishSnapshot(pendingSnapshot);
      if (!bound) return "staged";

      applyPending(bound.api);
      return "applied";
    },

    captureBoundSnapshot() {
      if (bound) capture(bound.api);
    },

    getLatestSnapshot() {
      return cloneLayout(latestSnapshot);
    },

    resetToDefault() {
      restoreEpoch += 1;
      pendingSnapshot = cloneLayout(defaultSnapshot);
      publishSnapshot(pendingSnapshot);
      if (bound) applyPending(bound.api);
    },
  };

  return runtime;
}

export const logsLayoutRuntime = createLogsLayoutRuntime();
