import { create } from 'zustand';
import type { Pin } from '@/shared/types/domain';
import type { SidebarDetailFocus } from '../detail/types';

interface ContextMenuState {
  x: number;
  y: number;
  visible: boolean;
}

interface EditorStore {
  contextMenu: ContextMenuState | null;
  setContextMenu: (menu: ContextMenuState | null) => void;

  sidebarDetailFocus: SidebarDetailFocus | null;
  setSidebarDetailFocus: (focus: SidebarDetailFocus) => void;
  clearSidebarDetailFocus: () => void;

  pendingConnection: Pin | null;
  setPendingConnection: (pin: Pin | null) => void;
}

export const useEditorStore = create<EditorStore>((set) => ({
  contextMenu: null,
  setContextMenu: (menu) => set({ contextMenu: menu }),

  sidebarDetailFocus: null,
  setSidebarDetailFocus: (focus) => set({ sidebarDetailFocus: focus }),
  clearSidebarDetailFocus: () => set({ sidebarDetailFocus: null }),

  pendingConnection: null,
  setPendingConnection: (pin) => set({ pendingConnection: pin }),
}));
