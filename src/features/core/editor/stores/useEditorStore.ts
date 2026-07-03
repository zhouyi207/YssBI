import { create } from 'zustand';
import type { Pin } from '@/shared/types/domain';
import type { DetailFocus } from '../detail/types';

interface ContextMenuState {
  x: number;
  y: number;
  visible: boolean;
}

interface EditorStore {
  contextMenu: ContextMenuState | null;
  setContextMenu: (menu: ContextMenuState | null) => void;

  detailFocus: DetailFocus | null;
  setDetailFocus: (focus: DetailFocus) => void;
  clearDetailFocus: () => void;

  /** Stable graph id for Variables sidebar local scope — survives tab close. */
  variablesGraphScopeId: string | null;
  setVariablesGraphScope: (graphId: string | null) => void;

  pendingConnection: Pin | null;
  setPendingConnection: (pin: Pin | null) => void;
}

export const useEditorStore = create<EditorStore>((set) => ({
  contextMenu: null,
  setContextMenu: (menu) => set({ contextMenu: menu }),

  detailFocus: null,
  setDetailFocus: (focus) => set({ detailFocus: focus }),
  clearDetailFocus: () => set({ detailFocus: null }),

  variablesGraphScopeId: null,
  setVariablesGraphScope: (graphId) => set({ variablesGraphScopeId: graphId }),

  pendingConnection: null,
  setPendingConnection: (pin) => set({ pendingConnection: pin }),
}));
