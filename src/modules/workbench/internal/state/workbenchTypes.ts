/** Non-placement workbench UI state; the root Dockview owns layout and visibility. */
export interface WorkbenchUiState {
  isSettingsOpen: boolean;
  isNodeDocumentationOpen: boolean;
}

export interface WorkbenchUiCommands {
  openSettings(): void;
  closeSettings(): void;
  setSettingsOpen(open: boolean): void;
  setNodeDocumentationOpen(open: boolean): void;
  resetWorkbenchUiState(): void;
}

export type WorkbenchUiStore = WorkbenchUiState & WorkbenchUiCommands;
