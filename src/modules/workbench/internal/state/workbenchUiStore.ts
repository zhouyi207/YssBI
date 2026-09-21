import { create } from "zustand";
import type { WorkbenchUiState, WorkbenchUiStore } from "./workbenchTypes";

/** Default non-placement UI state; the root FlexLayout owns layout and visibility. */
export const DEFAULT_WORKBENCH_UI_STATE: Readonly<WorkbenchUiState> = {
  isNodeDocumentationOpen: false,
};

export const useWorkbenchUiStore = create<WorkbenchUiStore>((set) => ({
  ...DEFAULT_WORKBENCH_UI_STATE,

  setNodeDocumentationOpen: (isNodeDocumentationOpen: boolean) => set({ isNodeDocumentationOpen }),
  resetWorkbenchUiState: () => set(DEFAULT_WORKBENCH_UI_STATE),
}));
