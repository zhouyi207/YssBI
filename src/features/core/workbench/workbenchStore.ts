import { create } from 'zustand';
import type {
  SidebarTabId,
  WorkbenchStore,
  WorkbenchUIState,
} from './workbenchTypes';

/** Default non-layout workbench UI state; Dockview remains authoritative for placement. */
export const DEFAULT_WORKBENCH_UI_STATE: Readonly<WorkbenchUIState> = {
  sidebarCurrentTab: 'graphs',
  sidebarUserHidden: false,
  panelCollapsed: false,
  detailUserHidden: false,
  isSettingsOpen: false,
  isNodeDocumentationOpen: false,
  zenMode: false,
};

export const useWorkbenchStore = create<WorkbenchStore>((set) => ({
  ...DEFAULT_WORKBENCH_UI_STATE,

  setSidebarCurrentTab: (sidebarCurrentTab: SidebarTabId) => set({ sidebarCurrentTab }),
  showSidebarTab: (sidebarCurrentTab: SidebarTabId) =>
    set({ sidebarCurrentTab, sidebarUserHidden: false }),
  toggleSidebarTab: (tab: SidebarTabId) =>
    set((state) => {
      if (state.sidebarCurrentTab === tab && !state.sidebarUserHidden) {
        return { sidebarUserHidden: true };
      }
      return { sidebarCurrentTab: tab, sidebarUserHidden: false };
    }),
  setSidebarUserHidden: (sidebarUserHidden: boolean) => set({ sidebarUserHidden }),
  toggleSidebarVisibilityPreference: () =>
    set((state) => ({ sidebarUserHidden: !state.sidebarUserHidden })),

  setPanelCollapsed: (panelCollapsed: boolean) => set({ panelCollapsed }),
  togglePanelCollapsed: () =>
    set((state) => ({ panelCollapsed: !state.panelCollapsed })),

  setDetailUserHidden: (detailUserHidden: boolean) => set({ detailUserHidden }),
  toggleDetailVisibilityPreference: () =>
    set((state) => ({ detailUserHidden: !state.detailUserHidden })),

  openSettings: () => set({ isSettingsOpen: true }),
  closeSettings: () => set({ isSettingsOpen: false }),
  setSettingsOpen: (isSettingsOpen: boolean) => set({ isSettingsOpen }),
  setNodeDocumentationOpen: (isNodeDocumentationOpen: boolean) =>
    set({ isNodeDocumentationOpen }),

  enterZenMode: () => set({ zenMode: true }),
  exitZenMode: () => set({ zenMode: false }),
  toggleZenMode: () => set((state) => ({ zenMode: !state.zenMode })),

  resetWorkbenchUIState: () => set(DEFAULT_WORKBENCH_UI_STATE),
}));
