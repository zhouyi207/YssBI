import { create } from "zustand";
import type { WorkbenchUiState, WorkbenchUiStore } from "./workbenchTypes";

/** Default non-placement UI state; the root Dockview owns layout and visibility. */
export const DEFAULT_WORKBENCH_UI_STATE: Readonly<WorkbenchUiState> = {
  isSettingsOpen: false,
  isNodeDocumentationOpen: false,
};

export const useWorkbenchUiStore = create<WorkbenchUiStore>((set) => ({
  ...DEFAULT_WORKBENCH_UI_STATE,

  openSettings: () => set({ isSettingsOpen: true }),
  closeSettings: () => set({ isSettingsOpen: false }),
  setSettingsOpen: (isSettingsOpen: boolean) => set({ isSettingsOpen }),
  setNodeDocumentationOpen: (isNodeDocumentationOpen: boolean) => set({ isNodeDocumentationOpen }),
  resetWorkbenchUiState: () => set(DEFAULT_WORKBENCH_UI_STATE),
}));
