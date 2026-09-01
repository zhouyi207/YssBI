import { useSyncExternalStore } from "react";

import type { DeepReadonly } from "@/shared/types/deepReadonly";
import { useWorkbenchStore } from "./workbenchStore";
import type { WorkbenchUIState } from "./workbenchTypes";

export interface WorkbenchUiCapability {
  readonly getSnapshot: () => DeepReadonly<WorkbenchUIState>;
  readonly subscribe: (listener: () => void) => () => void;
  readonly setSettingsOpen: (open: boolean) => void;
  readonly setNodeDocumentationOpen: (open: boolean) => void;
  readonly openSettings: () => void;
}

function snapshot(): DeepReadonly<WorkbenchUIState> {
  const state = useWorkbenchStore.getState();
  return Object.freeze({
    isSettingsOpen: state.isSettingsOpen,
    isNodeDocumentationOpen: state.isNodeDocumentationOpen,
  });
}

let currentSnapshot = snapshot();
const listeners = new Set<() => void>();

useWorkbenchStore.subscribe(() => {
  currentSnapshot = snapshot();
  for (const listener of listeners) listener();
});

export function getWorkbenchUiSnapshot(): DeepReadonly<WorkbenchUIState> {
  return currentSnapshot;
}

export function subscribeWorkbenchUi(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useWorkbenchUi<T>(selector: (state: DeepReadonly<WorkbenchUIState>) => T): T {
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
  setSettingsOpen: (open) => useWorkbenchStore.getState().setSettingsOpen(open),
  setNodeDocumentationOpen: (open) => useWorkbenchStore.getState().setNodeDocumentationOpen(open),
  openSettings: () => useWorkbenchStore.getState().openSettings(),
};
