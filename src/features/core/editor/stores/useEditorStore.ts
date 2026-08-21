import { create } from 'zustand';

import type { DetailFocus } from '../detail/types';

interface ContextMenuState {
  x: number;
  y: number;
  visible: boolean;
}

export type RightSidebarTab = 'details' | 'inspect' | 'result';

interface EditorStore {
  contextMenu: ContextMenuState | null;
  setContextMenu: (menu: ContextMenuState | null) => void;
  detailFocus: DetailFocus | null;
  setDetailFocus: (focus: DetailFocus) => void;
  clearDetailFocus: () => void;
  rightSidebarTab: RightSidebarTab;
  setRightSidebarTab: (tab: RightSidebarTab) => void;
  resetRightSidebar: () => void;
  variablesGraphScopePath: string | null;
  setVariablesGraphScope: (graphPath: string | null) => void;
}

export const useEditorStore = create<EditorStore>((set) => ({
  contextMenu: null,
  setContextMenu: (menu) => set({ contextMenu: menu }),
  detailFocus: null,
  setDetailFocus: (detailFocus) => set({ detailFocus }),
  clearDetailFocus: () => set({ detailFocus: null }),
  rightSidebarTab: 'details',
  setRightSidebarTab: (rightSidebarTab) => set({ rightSidebarTab }),
  resetRightSidebar: () => set({ detailFocus: null, rightSidebarTab: 'details' }),
  variablesGraphScopePath: null,
  setVariablesGraphScope: (variablesGraphScopePath) => set({ variablesGraphScopePath }),
}));
