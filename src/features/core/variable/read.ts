import { useSyncExternalStore } from 'react';

import type { DeepReadonly } from '@/features/core/projection/deepReadonly';
import { freezeProjectionSnapshot } from '@/features/core/projection/deepReadonly';
import { useVariableStore } from '@/features/core/dataStore/variableStore';
import type { VariableId, Variable } from '@/shared/types/domain';

export interface VariableReadSnapshot {
  readonly variables: DeepReadonly<Record<VariableId, Variable>>;
  readonly revisions: DeepReadonly<Record<VariableId, number>>;
}

export interface VariableReadCapability {
  readonly getSnapshot: () => VariableReadSnapshot;
  readonly subscribe: (listener: () => void) => () => void;
}

function buildSnapshot(): VariableReadSnapshot {
  const state = useVariableStore.getState();
  return freezeProjectionSnapshot({
    variables: state.variables,
    revisions: state.revisions,
  });
}

let currentSnapshot = buildSnapshot();
const listeners = new Set<() => void>();

function refreshSnapshot(): void {
  currentSnapshot = buildSnapshot();
  for (const listener of listeners) listener();
}

useVariableStore.subscribe(refreshSnapshot);

export function getVariableSnapshot(): VariableReadSnapshot {
  return currentSnapshot;
}

export function subscribeVariableRead(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useVariableRead<T>(
  selector: (snapshot: VariableReadSnapshot) => T,
): T {
  const snapshot = useSyncExternalStore(
    subscribeVariableRead,
    getVariableSnapshot,
    getVariableSnapshot,
  );
  return selector(snapshot);
}

export const variableRead: VariableReadCapability = {
  getSnapshot: getVariableSnapshot,
  subscribe: subscribeVariableRead,
};
