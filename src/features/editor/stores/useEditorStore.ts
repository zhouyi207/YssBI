import { create } from 'zustand';

interface ContextMenuState {
  x: number;
  y: number;
  visible: boolean;
}

interface EditorStore {
  // Context Menu
  contextMenu: ContextMenuState | null;
  setContextMenu: (menu: ContextMenuState | null) => void;

  // Selection State
  selectedItemId: string | null;
  selectedItemType: 'variable' | 'event' | 'function' | 'macro' | 'data' | 'setting' | null;
  setSelectedInfo: (id: string | null, type: 'variable' | 'event' | 'function' | 'macro' | 'data' | 'setting' | null) => void;

  // Pending Connection (for pin dragging)
  pendingConnection: any | null;
  setPendingConnection: (pin: any | null) => void;
}

export const useEditorStore = create<EditorStore>((set) => ({
  // Context Menu
  contextMenu: null,
  setContextMenu: (menu) => set({ contextMenu: menu }),

  // Selection
  selectedItemId: null,
  selectedItemType: null,
  setSelectedInfo: (id, type) => set({ selectedItemId: id, selectedItemType: type }),

  // Pending Connection
  pendingConnection: null,
  setPendingConnection: (pin) => set({ pendingConnection: pin }),
}));
