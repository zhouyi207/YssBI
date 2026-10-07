import type { IJsonModel } from "flexlayout-react";
import { createStore } from "zustand/vanilla";
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
  const published = createStore(() => ({ snapshot: defaultSnapshot }));
  let pendingSnapshot: IJsonModel | undefined = defaultSnapshot;
  let restoreEpoch = 0;
  let bound: BoundFlexLayout | undefined;

  const publishSnapshot = (layout: IJsonModel, owned = false): void => {
    if (snapshotsEqual(published.getState().snapshot, layout)) return;

    // Restore candidates are already isolated. Native toJson may share node config.
    published.setState({ snapshot: owned ? layout : cloneLayout(layout) });
  };

  const capture = (binding: BoundFlexLayout): void => {
    const layout = binding.api.getModel().toJson();
    if (bound === binding) publishSnapshot(layout);
  };

  const applyPending = (binding: BoundFlexLayout): void => {
    const pending = pendingSnapshot;
    if (!pending) return;

    binding.api.replace(pending);
    if (bound === binding && pendingSnapshot === pending) {
      pendingSnapshot = undefined;
    }
  };

  const runtime: LogsLayoutRuntime = {
    bind(api) {
      if (bound?.api === api) return;
      if (bound) runtime.unbind(bound.api);
      // Final capture can synchronously install a successor through an observer.
      if (bound) return;

      const binding: BoundFlexLayout = {
        api,
        dispose: api.subscribe(() => {
          if (bound === binding) capture(binding);
        }),
      };
      bound = binding;
      try {
        applyPending(binding);
      } catch (error) {
        if (bound === binding) {
          bound = undefined;
          binding.dispose();
        }
        throw error;
      }
    },

    unbind(api) {
      const current = bound;
      if (!current || (api !== undefined && api !== current.api)) return;

      let failure: { readonly value: unknown } | undefined;
      try {
        capture(current);
      } catch (error) {
        failure = { value: error };
      } finally {
        if (bound === current) {
          pendingSnapshot = published.getState().snapshot;
          bound = undefined;
        }
        try {
          current.dispose();
        } catch (error) {
          failure ??= { value: error };
        }
      }
      if (failure) throw failure.value;
    },

    subscribe(listener) {
      return published.subscribe(() => {
        try {
          listener();
        } catch {
          // Observer failures must not interrupt layout lifecycle transitions.
        }
      });
    },

    beginRestore() {
      restoreEpoch += 1;
      return restoreEpoch;
    },

    stageRestore(epoch, layout) {
      if (epoch !== restoreEpoch) return "stale";

      pendingSnapshot = cloneLayout(layout);
      publishSnapshot(pendingSnapshot, true);
      if (!bound) return "staged";

      applyPending(bound);
      return "applied";
    },

    captureBoundSnapshot() {
      if (bound) capture(bound);
    },

    getLatestSnapshot() {
      return cloneLayout(published.getState().snapshot);
    },

    resetToDefault() {
      restoreEpoch += 1;
      pendingSnapshot = cloneLayout(defaultSnapshot);
      publishSnapshot(pendingSnapshot, true);
      if (bound) applyPending(bound);
    },
  };

  return runtime;
}

export const logsLayoutRuntime = createLogsLayoutRuntime();
