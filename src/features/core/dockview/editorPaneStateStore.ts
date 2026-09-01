import { create } from "zustand";

type EditorPanePanelId = string;

export interface EditorPaneSelection {
  selectedNodeIds: string[];
  selectedConnectionIds: string[];
}

export const EMPTY_EDITOR_PANE_SELECTION: EditorPaneSelection = {
  selectedNodeIds: [],
  selectedConnectionIds: [],
};

interface EditorPaneState {
  selections: Record<EditorPanePanelId, EditorPaneSelection>;
  setSelectedNodeIds(panelInstanceId: EditorPanePanelId, ids: string[]): void;
  setSelectedConnectionIds(panelInstanceId: EditorPanePanelId, ids: string[]): void;
  clearSelection(panelInstanceId: EditorPanePanelId): void;
  release(panelInstanceId: EditorPanePanelId): void;
  reset(): void;
}

const unique = (ids: readonly string[]): string[] => [...new Set(ids)];

/** Pane-local UI state only; Dockview remains authoritative for panel placement. */
export const useEditorPaneStateStore = create<EditorPaneState>((set) => ({
  selections: {},
  setSelectedNodeIds: (panelInstanceId, ids) =>
    set((state) => ({
      selections: {
        ...state.selections,
        [panelInstanceId]: { selectedNodeIds: unique(ids), selectedConnectionIds: [] },
      },
    })),
  setSelectedConnectionIds: (panelInstanceId, ids) =>
    set((state) => ({
      selections: {
        ...state.selections,
        [panelInstanceId]: { selectedNodeIds: [], selectedConnectionIds: unique(ids) },
      },
    })),
  clearSelection: (panelInstanceId) =>
    set((state) => ({
      selections: { ...state.selections, [panelInstanceId]: { ...EMPTY_EDITOR_PANE_SELECTION } },
    })),
  release: (panelInstanceId) =>
    set((state) => {
      const selections = { ...state.selections };
      delete selections[panelInstanceId];
      return { selections };
    }),
  reset: () => set({ selections: {} }),
}));

export function getPaneSelection(
  panelInstanceId: EditorPanePanelId | undefined,
): EditorPaneSelection {
  if (!panelInstanceId) return EMPTY_EDITOR_PANE_SELECTION;
  return (
    useEditorPaneStateStore.getState().selections[panelInstanceId] ?? EMPTY_EDITOR_PANE_SELECTION
  );
}
