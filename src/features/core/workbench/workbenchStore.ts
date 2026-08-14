import { create } from 'zustand';
import type {
  PanelViewId,
  SidebarTabId,
  WorkbenchStore,
  WorkbenchUIState,
} from './workbenchTypes';

/** Matches the legacy workbench chrome defaults without copying layout data. */
export const DEFAULT_WORKBENCH_UI_STATE: Readonly<WorkbenchUIState> = {
  sidebarCurrentTab: 'graphs',
  sidebarUserHidden: false,
  panelActiveView: 'logs',
  panelUserHidden: false,
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

  setPanelActiveView: (panelActiveView: PanelViewId) => set({ panelActiveView }),
  setPanelUserHidden: (panelUserHidden: boolean) => set({ panelUserHidden }),
  togglePanelVisibilityPreference: () =>
    set((state) => ({ panelUserHidden: !state.panelUserHidden })),

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
