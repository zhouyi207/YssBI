import { useSyncExternalStore } from "react";

import type { DeepReadonly } from "@/shared/types/deepReadonly";
import { useWorkbenchUiStore } from "./workbenchUiStore";
import type { WorkbenchUiState } from "./workbenchTypes";

export interface WorkbenchUiCapability {
  readonly getSnapshot: () => DeepReadonly<WorkbenchUiState>;
  readonly subscribe: (listener: () => void) => () => void;
  readonly setSettingsOpen: (open: boolean) => void;
  readonly setNodeDocumentationOpen: (open: boolean) => void;
  readonly openSettings: () => void;
}

function snapshot(): DeepReadonly<WorkbenchUiState> {
  const state = useWorkbenchUiStore.getState();
  return Object.freeze({
    isSettingsOpen: state.isSettingsOpen,
    isNodeDocumentationOpen: state.isNodeDocumentationOpen,
  });
}

let currentSnapshot = snapshot();
const listeners = new Set<() => void>();

useWorkbenchUiStore.subscribe(() => {
  currentSnapshot = snapshot();
  for (const listener of listeners) listener();
});

export function getWorkbenchUiSnapshot(): DeepReadonly<WorkbenchUiState> {
  return currentSnapshot;
}

export function subscribeWorkbenchUi(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useWorkbenchUi<T>(selector: (state: DeepReadonly<WorkbenchUiState>) => T): T {
  const state = useSyncExternalStore(
    subscribeWorkbenchUi,
    getWorkbenchUiSnapshot,
    getWorkbenchUiSnapshot,
  );
  return selector(state);
}

export const workbenchUi: WorkbenchUiCapability = {
  getSnapshot: getWorkbenchUiSnapshot,
  subscribe: subscribeWorkbenchUi,
  setSettingsOpen: (open) => useWorkbenchUiStore.getState().setSettingsOpen(open),
  setNodeDocumentationOpen: (open) => useWorkbenchUiStore.getState().setNodeDocumentationOpen(open),
  openSettings: () => useWorkbenchUiStore.getState().openSettings(),
};
