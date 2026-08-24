import { create } from 'zustand';

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
  variablesGraphScopePath: string | null;
  setVariablesGraphScope: (graphPath: string | null) => void;
}

export const useEditorStore = create<EditorStore>((set) => ({
  contextMenu: null,
  setContextMenu: (menu) => set({ contextMenu: menu }),
  detailFocus: null,
  setDetailFocus: (detailFocus) => set({ detailFocus }),
  clearDetailFocus: () => set({ detailFocus: null }),
  variablesGraphScopePath: null,
  setVariablesGraphScope: (variablesGraphScopePath) => set({ variablesGraphScopePath }),
}));
