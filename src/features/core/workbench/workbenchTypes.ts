/** Non-placement workbench UI state; the root Dockview owns layout and visibility. */
export interface WorkbenchUIState {
  isSettingsOpen: boolean;
  isNodeDocumentationOpen: boolean;
}

export interface WorkbenchUICommands {
  openSettings(): void;
  closeSettings(): void;
  setSettingsOpen(open: boolean): void;
  setNodeDocumentationOpen(open: boolean): void;
  resetWorkbenchUIState(): void;
}

export type WorkbenchStore = WorkbenchUIState & WorkbenchUICommands;
