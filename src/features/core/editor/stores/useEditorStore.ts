import { create } from 'zustand';

import type { DetailFocus } from '../detail/types';

interface ContextMenuState {
  x: number;
  y: number;
  visible: boolean;
}

export type DetailPaneTab = 'details' | 'inspector';

interface EditorStore {
  contextMenu: ContextMenuState | null;
  setContextMenu: (menu: ContextMenuState | null) => void;

  detailFocus: DetailFocus | null;
  setDetailFocus: (focus: DetailFocus) => void;
  clearDetailFocus: () => void;

  detailPaneTab: DetailPaneTab;
  inspectorResultId: string | null;
  setDetailPaneTab: (tab: DetailPaneTab) => void;
  inspectResult: (resultId: string) => void;
  clearInspectorResult: () => void;

  /** 变量侧栏 Local scope 锚定的图 path；关闭 tab 后仍保留。 */
  variablesGraphScopePath: string | null;
  setVariablesGraphScope: (graphPath: string | null) => void;

}

export const useEditorStore = create<EditorStore>((set) => ({
  contextMenu: null,
  setContextMenu: (menu) => set({ contextMenu: menu }),

  detailFocus: null,
  setDetailFocus: (focus) => set({ detailFocus: focus }),
  clearDetailFocus: () => set({ detailFocus: null }),

  detailPaneTab: 'details',
  inspectorResultId: null,
  setDetailPaneTab: (detailPaneTab) => set({ detailPaneTab }),
  inspectResult: (inspectorResultId) => set({
    detailPaneTab: 'inspector',
    inspectorResultId,
  }),
  clearInspectorResult: () => set({ inspectorResultId: null }),

  variablesGraphScopePath: null,
  setVariablesGraphScope: (graphPath) => set({ variablesGraphScopePath: graphPath }),
}));
