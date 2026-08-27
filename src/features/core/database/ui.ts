import { useSyncExternalStore } from 'react';
import { create } from 'zustand';
import {
  freezeProjectionSnapshot,
  type DeepReadonly,
} from '@/features/core/projection/deepReadonly';
import type { DatabaseId } from '@/shared/types/domain/ids';

export interface DatabaseCopyFocus {
  readonly rowIndex: number;
  readonly columnIndex: number;
}

export interface DatabaseUiSnapshot {
  readonly selectedDatabaseId: DatabaseId | null;
  readonly queryByDatabase: DeepReadonly<Record<DatabaseId, string>>;
  readonly pageByDatabase: DeepReadonly<Record<DatabaseId, number>>;
  readonly copyFocusByDatabase: DeepReadonly<Record<DatabaseId, DatabaseCopyFocus | null>>;
}

interface DatabaseUiStore extends DatabaseUiSnapshot {
  selectDatabase(id: DatabaseId | null): void;
  setQuery(id: DatabaseId, query: string): void;
  setPage(id: DatabaseId, page: number): void;
  setCopyFocus(id: DatabaseId, focus: DatabaseCopyFocus | null): void;
  resetDatabase(id: DatabaseId): void;
  resetForProject(): void;
}

export interface DatabaseUiCapability {
  readonly getSnapshot: () => DeepReadonly<DatabaseUiSnapshot>;
  readonly subscribe: (listener: () => void) => () => void;
  readonly selectDatabase: (id: DatabaseId | null) => void;
  readonly setQuery: (id: DatabaseId, query: string) => void;
  readonly setPage: (id: DatabaseId, page: number) => void;
  readonly setCopyFocus: (id: DatabaseId, focus: DatabaseCopyFocus | null) => void;
  readonly resetDatabase: (id: DatabaseId) => void;
  readonly resetForProject: () => void;
}

const useDatabaseUiStore = create<DatabaseUiStore>((set) => ({
  selectedDatabaseId: null,
  queryByDatabase: {},
  pageByDatabase: {},
  copyFocusByDatabase: {},

  selectDatabase: (id) => set({ selectedDatabaseId: id }),
  setQuery: (id, query) => set((state) => ({
    queryByDatabase: { ...state.queryByDatabase, [id]: query },
  })),
  setPage: (id, page) => set((state) => ({
    pageByDatabase: { ...state.pageByDatabase, [id]: Math.max(0, page) },
  })),
  setCopyFocus: (id, focus) => set((state) => ({
    copyFocusByDatabase: { ...state.copyFocusByDatabase, [id]: focus },
  })),
  resetDatabase: (id) => set((state) => {
    const queryByDatabase = { ...state.queryByDatabase };
    const pageByDatabase = { ...state.pageByDatabase };
    const copyFocusByDatabase = { ...state.copyFocusByDatabase };
    delete queryByDatabase[id];
    delete pageByDatabase[id];
    delete copyFocusByDatabase[id];
    return {
      selectedDatabaseId: state.selectedDatabaseId === id ? null : state.selectedDatabaseId,
      queryByDatabase,
      pageByDatabase,
      copyFocusByDatabase,
    };
  }),
  resetForProject: () => set({
    selectedDatabaseId: null,
    queryByDatabase: {},
    pageByDatabase: {},
    copyFocusByDatabase: {},
  }),
}));

function buildSnapshot(): DeepReadonly<DatabaseUiSnapshot> {
  const state = useDatabaseUiStore.getState();
  return freezeProjectionSnapshot({
    selectedDatabaseId: state.selectedDatabaseId,
    queryByDatabase: state.queryByDatabase,
    pageByDatabase: state.pageByDatabase,
    copyFocusByDatabase: state.copyFocusByDatabase,
  });
}

let currentSnapshot = buildSnapshot();
const listeners = new Set<() => void>();
useDatabaseUiStore.subscribe(() => {
  currentSnapshot = buildSnapshot();
  for (const listener of listeners) listener();
});

export function getDatabaseUiSnapshot(): DeepReadonly<DatabaseUiSnapshot> {
  return currentSnapshot;
}

export function subscribeDatabaseUi(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useDatabaseUi<T>(
  selector: (snapshot: DeepReadonly<DatabaseUiSnapshot>) => T,
): T {
  const snapshot = useSyncExternalStore(
    subscribeDatabaseUi,
    getDatabaseUiSnapshot,
    getDatabaseUiSnapshot,
  );
  return selector(snapshot);
}

export const databaseUi: DatabaseUiCapability = {
  getSnapshot: getDatabaseUiSnapshot,
  subscribe: subscribeDatabaseUi,
  selectDatabase: (id) => useDatabaseUiStore.getState().selectDatabase(id),
  setQuery: (id, query) => useDatabaseUiStore.getState().setQuery(id, query),
  setPage: (id, page) => useDatabaseUiStore.getState().setPage(id, page),
  setCopyFocus: (id, focus) => useDatabaseUiStore.getState().setCopyFocus(id, focus),
  resetDatabase: (id) => useDatabaseUiStore.getState().resetDatabase(id),
  resetForProject: () => useDatabaseUiStore.getState().resetForProject(),
};
