import { createReadProjection, useReadProjection } from "@/features/core/state/readProjection";

import type { DeepReadonly } from "@/shared/types/deepReadonly";
import { useWorkbenchUiStore } from "./workbenchUiStore";
import type { WorkbenchUiState } from "./workbenchTypes";

export interface WorkbenchUiCapability {
  readonly getSnapshot: () => DeepReadonly<WorkbenchUiState>;
  readonly subscribe: (listener: () => void) => () => void;
  readonly setNodeDocumentationOpen: (open: boolean) => void;
}

function snapshot(): DeepReadonly<WorkbenchUiState> {
  const state = useWorkbenchUiStore.getState();
  return {
    isNodeDocumentationOpen: state.isNodeDocumentationOpen,
  };
}

const projection = createReadProjection(snapshot, [useWorkbenchUiStore]);
export const getWorkbenchUiSnapshot = projection.getSnapshot;
export const subscribeWorkbenchUi = projection.subscribe;
export function useWorkbenchUi<T>(selector: (state: DeepReadonly<WorkbenchUiState>) => T): T {
  return useReadProjection(projection, selector);
}

export const workbenchUi: WorkbenchUiCapability = {
  getSnapshot: getWorkbenchUiSnapshot,
  subscribe: subscribeWorkbenchUi,
  setNodeDocumentationOpen: (open) => useWorkbenchUiStore.getState().setNodeDocumentationOpen(open),
};
