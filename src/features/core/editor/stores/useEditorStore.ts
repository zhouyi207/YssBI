import { create } from 'zustand';

import type { DetailFocus } from '../detail/types';

export interface EditorContextMenuState {
  x: number;
  y: number;
  visible: boolean;
  panelInstanceId?: string;
  groupId?: string;
  graphPath?: string;
}

interface EditorStore {
  contextMenu: EditorContextMenuState | null;
  setContextMenu: (menu: EditorContextMenuState | null) => void;
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
