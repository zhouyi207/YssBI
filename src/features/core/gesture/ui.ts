import { useSyncExternalStore } from 'react';

import type { DeepReadonly } from '@/shared/types/deepReadonly';
import { useGestureStore } from './useGestureStore';

export interface GestureUiSnapshot {
  readonly suppressNextContextMenu: boolean;
}

export interface GestureUiCapability {
  readonly getSnapshot: () => DeepReadonly<GestureUiSnapshot>;
  readonly subscribe: (listener: () => void) => () => void;
  readonly clearGesture: (hadMovement?: boolean) => void;
  readonly consumeSuppressContextMenu: () => boolean;
}

function buildSnapshot(): DeepReadonly<GestureUiSnapshot> {
  return Object.freeze({
    suppressNextContextMenu: useGestureStore.getState().suppressNextContextMenu,
  });
}

let currentSnapshot = buildSnapshot();
const listeners = new Set<() => void>();

function refreshSnapshot(): void {
  currentSnapshot = buildSnapshot();
  for (const listener of listeners) listener();
}

useGestureStore.subscribe(refreshSnapshot);

export function getGestureUiSnapshot(): DeepReadonly<GestureUiSnapshot> {
  return currentSnapshot;
}

export function subscribeGestureUi(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useGestureUi<T>(
  selector: (snapshot: DeepReadonly<GestureUiSnapshot>) => T,
): T {
  const snapshot = useSyncExternalStore(
    subscribeGestureUi,
    getGestureUiSnapshot,
    getGestureUiSnapshot,
  );
  return selector(snapshot);
}

export const gestureUi: GestureUiCapability = {
  getSnapshot: getGestureUiSnapshot,
  subscribe: subscribeGestureUi,
  clearGesture: (hadMovement) => useGestureStore.getState().clearGesture(hadMovement),
  consumeSuppressContextMenu: () =>
    useGestureStore.getState().consumeSuppressContextMenu(),
};
