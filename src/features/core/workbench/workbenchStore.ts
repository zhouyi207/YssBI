import { create } from "zustand";
import type { WorkbenchStore, WorkbenchUIState } from "./workbenchTypes";

/** Default non-placement UI state; the root Dockview owns layout and visibility. */
export const DEFAULT_WORKBENCH_UI_STATE: Readonly<WorkbenchUIState> = {
  isSettingsOpen: false,
  isNodeDocumentationOpen: false,
};

export const useWorkbenchStore = create<WorkbenchStore>((set) => ({
  ...DEFAULT_WORKBENCH_UI_STATE,

  openSettings: () => set({ isSettingsOpen: true }),
  closeSettings: () => set({ isSettingsOpen: false }),
  setSettingsOpen: (isSettingsOpen: boolean) => set({ isSettingsOpen }),
  setNodeDocumentationOpen: (isNodeDocumentationOpen: boolean) => set({ isNodeDocumentationOpen }),
  resetWorkbenchUIState: () => set(DEFAULT_WORKBENCH_UI_STATE),
}));
