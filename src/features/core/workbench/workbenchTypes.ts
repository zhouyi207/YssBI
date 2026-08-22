export { PANEL_VIEW_IDS } from '@/features/core/layout/panelPartModel';
export type { PanelViewId } from '@/features/core/layout/panelPartModel';

export const SIDEBAR_TAB_IDS = ['project', 'nodes', 'data', 'commands'] as const;

export type SidebarTabId = (typeof SIDEBAR_TAB_IDS)[number];

export function isSidebarTabId(value: unknown): value is SidebarTabId {
  return typeof value === 'string'
    && (SIDEBAR_TAB_IDS as readonly string[]).includes(value);
}


/**
 * Non-layout-authoritative workbench UI state.
 *
 * Effective visibility and all geometry/topology remain owned by the layout
 * domain. This state captures sidebar/detail visibility and transient workbench UI state.
 */
export interface WorkbenchUIState {
  sidebarCurrentTab: SidebarTabId;
  sidebarUserHidden: boolean;
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
