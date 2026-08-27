import { useSyncExternalStore } from 'react';

import type { DeepReadonly } from '@/features/core/projection/deepReadonly';
import {
  useGraphSessionStore,
  type FocusedGraphSession,
} from './graphSessionStore';

export interface GraphSessionUiSnapshot {
  readonly focusedSession: DeepReadonly<FocusedGraphSession> | null;
}

export interface GraphSessionUiCapability {
  readonly getSnapshot: () => DeepReadonly<GraphSessionUiSnapshot>;
  readonly subscribe: (listener: () => void) => () => void;
  readonly setFocusedSession: (groupId: string, graphPath: string) => string | null;
  readonly clearFocusedSession: (groupId: string) => void;
  readonly remapFocusedGraphPath: (from: string, to: string) => void;
  readonly reset: () => void;
}

function buildSnapshot(): DeepReadonly<GraphSessionUiSnapshot> {
  const focusedSession = useGraphSessionStore.getState().focusedSession;
  return Object.freeze({
    focusedSession: focusedSession
      ? Object.freeze({ ...focusedSession })
      : null,
  });
}

let currentSnapshot = buildSnapshot();
const listeners = new Set<() => void>();

function refreshSnapshot(): void {
  currentSnapshot = buildSnapshot();
  for (const listener of listeners) listener();
}

useGraphSessionStore.subscribe(refreshSnapshot);

export function getGraphSessionUiSnapshot(): DeepReadonly<GraphSessionUiSnapshot> {
  return currentSnapshot;
}

export function subscribeGraphSessionUi(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useGraphSessionUi<T>(
  selector: (snapshot: DeepReadonly<GraphSessionUiSnapshot>) => T,
): T {
  const snapshot = useSyncExternalStore(
    subscribeGraphSessionUi,
    getGraphSessionUiSnapshot,
    getGraphSessionUiSnapshot,
  );
  return selector(snapshot);
}

export const graphSessionUi: GraphSessionUiCapability = {
  getSnapshot: getGraphSessionUiSnapshot,
  subscribe: subscribeGraphSessionUi,
  setFocusedSession: (groupId, graphPath) =>
    useGraphSessionStore.getState().setFocusedSession(groupId, graphPath),
  clearFocusedSession: (groupId) =>
    useGraphSessionStore.getState().clearFocusedSession(groupId),
  remapFocusedGraphPath: (from, to) =>
    useGraphSessionStore.getState().remapFocusedGraphPath(from, to),
  reset: () => useGraphSessionStore.getState().reset(),
};
