import { create } from 'zustand';

interface ContextMenuState {
  x: number;
  y: number;
  visible: boolean;
}

interface EditorStore {
  contextMenu: ContextMenuState | null;
  setContextMenu: (menu: ContextMenuState | null) => void;

  selectedItemId: string | null;
  selectedItemType: 'variable' | 'event' | 'function' | 'macro' | 'data' | 'setting' | null;
  setSelectedInfo: (id: string | null, type: 'variable' | 'event' | 'function' | 'macro' | 'data' | 'setting' | null) => void;

  pendingConnection: any | null;
  setPendingConnection: (pin: any | null) => void;
}

export const useEditorStore = create<EditorStore>((set) => ({
  contextMenu: null,
  setContextMenu: (menu) => set({ contextMenu: menu }),

  selectedItemId: null,
  selectedItemType: null,
  setSelectedInfo: (id, type) => set({ selectedItemId: id, selectedItemType: type }),

  pendingConnection: null,
  setPendingConnection: (pin) => set({ pendingConnection: pin }),
}));
