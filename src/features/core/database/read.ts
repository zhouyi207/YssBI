import { useSyncExternalStore } from 'react';

import type { DeepReadonly } from '@/shared/types/deepReadonly';
import { freezeProjectionSnapshot } from '@/shared/types/deepReadonly';
import { useDatabaseStore } from '@/features/core/dataStore/databaseStore';
import type { DatabaseRecord } from '@/shared/types/domain/database';

export interface DatabaseReadSnapshot {
  readonly databases: DeepReadonly<Record<string, DatabaseRecord>>;
  readonly revisions: DeepReadonly<Record<string, number>>;
}

export interface DatabaseReadCapability {
  readonly getSnapshot: () => DeepReadonly<DatabaseReadSnapshot>;
  readonly subscribe: (listener: () => void) => () => void;
}

function buildSnapshot(): DeepReadonly<DatabaseReadSnapshot> {
  const state = useDatabaseStore.getState();
  return freezeProjectionSnapshot({
    databases: state.databases,
    revisions: state.revisions,
  });
}

let currentSnapshot = buildSnapshot();
const listeners = new Set<() => void>();

function refreshSnapshot(): void {
  currentSnapshot = buildSnapshot();
  for (const listener of listeners) listener();
}

useDatabaseStore.subscribe(refreshSnapshot);

export function getDatabaseSnapshot(): DeepReadonly<DatabaseReadSnapshot> {
  return currentSnapshot;
}

export function subscribeDatabaseRead(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useDatabaseRead<T>(
  selector: (snapshot: DeepReadonly<DatabaseReadSnapshot>) => T,
): T {
  const snapshot = useSyncExternalStore(
    subscribeDatabaseRead,
    getDatabaseSnapshot,
    getDatabaseSnapshot,
  );
  return selector(snapshot);
}

export const databaseRead: DatabaseReadCapability = {
  getSnapshot: getDatabaseSnapshot,
  subscribe: subscribeDatabaseRead,
};
