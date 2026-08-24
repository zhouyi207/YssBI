export const SIDEBAR_TAB_IDS = ['project', 'nodes', 'data', 'commands'] as const;

export type SidebarTabId = (typeof SIDEBAR_TAB_IDS)[number];

/** Non-placement workbench UI state; the root Dockview owns layout and visibility. */
export interface WorkbenchUIState {
  sidebarCurrentTab: SidebarTabId;
  isSettingsOpen: boolean;
  isNodeDocumentationOpen: boolean;
}

export interface WorkbenchUICommands {
  setSidebarCurrentTab(tab: SidebarTabId): void;
  openSettings(): void;
  closeSettings(): void;
  setSettingsOpen(open: boolean): void;
  setNodeDocumentationOpen(open: boolean): void;
  resetWorkbenchUIState(): void;
}

export type WorkbenchStore = WorkbenchUIState & WorkbenchUICommands;
