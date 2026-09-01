import { useSyncExternalStore } from "react";
import { create } from "zustand";
import { freezeProjectionSnapshot, type DeepReadonly } from "@/shared/types/deepReadonly";
import type { DataValue } from "@/shared/types/domain/dataValue";
import type { VariableId, VariableScope } from "@/shared/types/domain";

export type VariableDraft = Partial<{
  name: string;
  dataValue: DataValue;
  description: string;
  scope: VariableScope;
  tags: string[];
}>;

export interface VariableUiSnapshot {
  readonly draftsById: DeepReadonly<Record<VariableId, VariableDraft>>;
  readonly scope: VariableScope | null;
}

interface VariableUiStore extends VariableUiSnapshot {
  setDraftValue(id: VariableId, value: DataValue): void;
  setScope(scope: VariableScope | null): void;
  resetDraft(id: VariableId): void;
  resetForProject(): void;
}

export interface VariableUiCapability {
  readonly getSnapshot: () => DeepReadonly<VariableUiSnapshot>;
  readonly subscribe: (listener: () => void) => () => void;
  readonly setDraftValue: (id: VariableId, value: DataValue) => void;
  readonly setScope: (scope: VariableScope | null) => void;
  readonly resetDraft: (id: VariableId) => void;
  readonly resetForProject: () => void;
}

const useVariableUiStore = create<VariableUiStore>((set) => ({
  draftsById: {},
  scope: null,

  setDraftValue: (id, value) =>
    set((state) => ({
      draftsById: {
        ...state.draftsById,
        [id]: { ...state.draftsById[id], dataValue: structuredClone(value) },
      },
    })),
  setScope: (scope) => set({ scope: scope ? structuredClone(scope) : null }),
  resetDraft: (id) =>
    set((state) => {
      const draftsById = { ...state.draftsById };
      delete draftsById[id];
      return { draftsById };
    }),
  resetForProject: () => set({ draftsById: {}, scope: null }),
}));

function buildSnapshot(): DeepReadonly<VariableUiSnapshot> {
  const state = useVariableUiStore.getState();
  return freezeProjectionSnapshot({
    draftsById: state.draftsById,
    scope: state.scope,
  });
}

let currentSnapshot = buildSnapshot();
const listeners = new Set<() => void>();
useVariableUiStore.subscribe(() => {
  currentSnapshot = buildSnapshot();
  for (const listener of listeners) listener();
});

export function getVariableUiSnapshot(): DeepReadonly<VariableUiSnapshot> {
  return currentSnapshot;
}

export function subscribeVariableUi(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useVariableUi<T>(selector: (snapshot: DeepReadonly<VariableUiSnapshot>) => T): T {
  const snapshot = useSyncExternalStore(
    subscribeVariableUi,
    getVariableUiSnapshot,
    getVariableUiSnapshot,
  );
  return selector(snapshot);
}

export const variableUi: VariableUiCapability = {
  getSnapshot: getVariableUiSnapshot,
  subscribe: subscribeVariableUi,
  setDraftValue: (id, value) => useVariableUiStore.getState().setDraftValue(id, value),
  setScope: (scope) => useVariableUiStore.getState().setScope(scope),
  resetDraft: (id) => useVariableUiStore.getState().resetDraft(id),
  resetForProject: () => useVariableUiStore.getState().resetForProject(),
};
