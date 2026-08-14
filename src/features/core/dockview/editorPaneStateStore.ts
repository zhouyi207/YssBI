import { create } from 'zustand';
import type { PanelInstanceId } from './types';

export interface EditorPaneSelection {
  selectedNodeIds: string[];
  selectedConnectionIds: string[];
}

interface EditorPaneState {
  selections: Record<PanelInstanceId, EditorPaneSelection>;
  setSelectedNodeIds(panelInstanceId: PanelInstanceId, ids: string[]): void;
  setSelectedConnectionIds(panelInstanceId: PanelInstanceId, ids: string[]): void;
  clearSelection(panelInstanceId: PanelInstanceId): void;
  release(panelInstanceId: PanelInstanceId): void;
  reset(): void;
}

const emptySelection = (): EditorPaneSelection => ({ selectedNodeIds: [], selectedConnectionIds: [] });
const unique = (ids: readonly string[]): string[] => [...new Set(ids)];

/** Pane-local UI state only; Dockview remains authoritative for panel placement. */
export const useEditorPaneStateStore = create<EditorPaneState>((set) => ({
  selections: {},
  setSelectedNodeIds: (panelInstanceId, ids) => set((state) => ({
    selections: {
      ...state.selections,
      [panelInstanceId]: { selectedNodeIds: unique(ids), selectedConnectionIds: [] },
    },
  })),
  setSelectedConnectionIds: (panelInstanceId, ids) => set((state) => ({
    selections: {
      ...state.selections,
      [panelInstanceId]: { selectedNodeIds: [], selectedConnectionIds: unique(ids) },
    },
  })),
  clearSelection: (panelInstanceId) => set((state) => ({
    selections: { ...state.selections, [panelInstanceId]: emptySelection() },
  })),
  release: (panelInstanceId) => set((state) => {
    const selections = { ...state.selections };
    delete selections[panelInstanceId];
    return { selections };
  }),
  reset: () => set({ selections: {} }),
}));

export function getPaneSelection(panelInstanceId: PanelInstanceId | undefined): EditorPaneSelection {
  if (!panelInstanceId) return emptySelection();
  return useEditorPaneStateStore.getState().selections[panelInstanceId] ?? emptySelection();
}
