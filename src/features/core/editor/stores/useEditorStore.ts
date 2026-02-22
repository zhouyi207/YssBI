import { create } from 'zustand';
import type { Pin } from '@/shared/types/domain';

interface ContextMenuState {
  x: number;
  y: number;
  visible: boolean;
}

interface EditorStore {
  contextMenu: ContextMenuState | null;
  setContextMenu: (menu: ContextMenuState | null) => void;

  selectedItemId: string | null;
  selectedItemType: 'variable' | 'event' | 'function' | 'macro' | 'data' | 'setting' | 'log' | null;
  setSelectedInfo: (id: string | null, type: 'variable' | 'event' | 'function' | 'macro' | 'data' | 'setting' | 'log' | null) => void;

  pendingConnection: Pin | null;
  setPendingConnection: (pin: Pin | null) => void;
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
