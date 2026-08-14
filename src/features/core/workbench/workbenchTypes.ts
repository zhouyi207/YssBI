export const SIDEBAR_TAB_IDS = [
  'graphs',
  'nodes',
  'variables',
  'data',
  'commands',
  'charts',
] as const;

export type SidebarTabId = (typeof SIDEBAR_TAB_IDS)[number];

export const PANEL_VIEW_IDS = ['logs', 'output', 'terminal'] as const;

export type PanelViewId = (typeof PANEL_VIEW_IDS)[number];

/**
 * Non-layout-authoritative workbench UI state.
 *
 * Effective visibility and all geometry/topology remain owned by the layout
 * domain. The `userHidden` fields capture only the user's visibility intent.
 */
export interface WorkbenchUIState {
  sidebarCurrentTab: SidebarTabId;
  sidebarUserHidden: boolean;
  panelActiveView: PanelViewId;
  panelUserHidden: boolean;
  detailUserHidden: boolean;
  isSettingsOpen: boolean;
  isNodeDocumentationOpen: boolean;
  zenMode: boolean;
}

export interface WorkbenchUICommands {
  setSidebarCurrentTab(tab: SidebarTabId): void;
  showSidebarTab(tab: SidebarTabId): void;
  toggleSidebarTab(tab: SidebarTabId): void;
  setSidebarUserHidden(hidden: boolean): void;
  toggleSidebarVisibilityPreference(): void;

  setPanelActiveView(view: PanelViewId): void;
  setPanelUserHidden(hidden: boolean): void;
  togglePanelVisibilityPreference(): void;

  setDetailUserHidden(hidden: boolean): void;
  toggleDetailVisibilityPreference(): void;

  openSettings(): void;
  closeSettings(): void;
  setSettingsOpen(open: boolean): void;
  setNodeDocumentationOpen(open: boolean): void;

  enterZenMode(): void;
  exitZenMode(): void;
  toggleZenMode(): void;

  resetWorkbenchUIState(): void;
}

export type WorkbenchStore = WorkbenchUIState & WorkbenchUICommands;
