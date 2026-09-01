import { useSyncExternalStore } from "react";

import type { DeepReadonly } from "@/shared/types/deepReadonly";
import { useHistoryStore, type HistoryStoreState } from "./historyStore";

export type HistoryProjectionSnapshot = DeepReadonly<HistoryStoreState>;

export interface HistoryReadCapability {
  readonly getSnapshot: () => HistoryProjectionSnapshot;
  readonly subscribe: (listener: () => void) => () => void;
}

function buildSnapshot(): HistoryProjectionSnapshot {
  const state = useHistoryStore.getState();
  return Object.freeze({
    canUndo: state.canUndo,
    canRedo: state.canRedo,
    pending: state.pending,
  });
}

let currentSnapshot = buildSnapshot();
const listeners = new Set<() => void>();

function refreshSnapshot(): void {
  currentSnapshot = buildSnapshot();
  for (const listener of listeners) listener();
}

useHistoryStore.subscribe(refreshSnapshot);

export function getHistorySnapshot(): HistoryProjectionSnapshot {
  return currentSnapshot;
}

export function subscribeHistoryRead(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useHistoryRead<T>(selector: (snapshot: HistoryProjectionSnapshot) => T): T {
  const snapshot = useSyncExternalStore(
    subscribeHistoryRead,
    getHistorySnapshot,
    getHistorySnapshot,
  );
  return selector(snapshot);
}

export const historyRead: HistoryReadCapability = {
  getSnapshot: getHistorySnapshot,
  subscribe: subscribeHistoryRead,
};
