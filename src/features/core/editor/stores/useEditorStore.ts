import { create } from 'zustand';
import type { Pin } from '@/shared/types/domain';

interface ContextMenuState {
  x: number;
  y: number;
  visible: boolean;
}

export type DetailSelectionType =
  | 'variable'
  | 'event'
  | 'function'
  | 'data'
  | 'setting'
  | 'log'
  | 'node'
  | 'worksheet';

interface EditorStore {
  contextMenu: ContextMenuState | null;
  setContextMenu: (menu: ContextMenuState | null) => void;

  selectedItemId: string | null;
  selectedItemType: DetailSelectionType | null;
  selectedGraphId: string | null;
  setSelectedInfo: (
    id: string | null,
    type: DetailSelectionType | null,
    graphId?: string | null,
  ) => void;

  pendingConnection: Pin | null;
  setPendingConnection: (pin: Pin | null) => void;
}

export const useEditorStore = create<EditorStore>((set) => ({
  contextMenu: null,
  setContextMenu: (menu) => set({ contextMenu: menu }),

  selectedItemId: null,
  selectedItemType: null,
  selectedGraphId: null,
  setSelectedInfo: (id, type, graphId = null) =>
    set({
      selectedItemId: id,
      selectedItemType: type,
      selectedGraphId: type === 'node' ? (graphId ?? null) : null,
    }),

  pendingConnection: null,
  setPendingConnection: (pin) => set({ pendingConnection: pin }),
}));
