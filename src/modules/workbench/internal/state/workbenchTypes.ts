/** Non-placement workbench UI state; the root FlexLayout owns layout and visibility. */
export interface WorkbenchUiState {
  isNodeDocumentationOpen: boolean;
}

export interface WorkbenchUiCommands {
  setNodeDocumentationOpen(open: boolean): void;
  resetWorkbenchUiState(): void;
}

export type WorkbenchUiStore = WorkbenchUiState & WorkbenchUiCommands;
