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
  collapsedNodeIds: Record<EditorPanePanelId, string[]>;
  setSelectedNodeIds(panelInstanceId: EditorPanePanelId, ids: string[]): void;
  setSelectedConnectionIds(panelInstanceId: EditorPanePanelId, ids: string[]): void;
  setNodeCollapsed(panelInstanceId: EditorPanePanelId, nodeId: string, collapsed: boolean): void;
  clearSelection(panelInstanceId: EditorPanePanelId): void;
  release(panelInstanceId: EditorPanePanelId): void;
  reset(): void;
}

const unique = (ids: readonly string[]): string[] => [...new Set(ids)];

/** Pane-local UI state only; FlexLayout remains authoritative for panel placement. */
export const useEditorPaneStateStore = create<EditorPaneState>((set) => ({
  selections: {},
  collapsedNodeIds: {},
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
  setNodeCollapsed: (panelInstanceId, nodeId, collapsed) =>
    set((state) => {
      const ids = new Set(state.collapsedNodeIds[panelInstanceId]);
      if (collapsed) ids.add(nodeId);
      else ids.delete(nodeId);
      return { collapsedNodeIds: { ...state.collapsedNodeIds, [panelInstanceId]: [...ids] } };
    }),
  clearSelection: (panelInstanceId) =>
    set((state) => ({
      selections: { ...state.selections, [panelInstanceId]: { ...EMPTY_EDITOR_PANE_SELECTION } },
    })),
  release: (panelInstanceId) =>
    set((state) => {
      const selections = { ...state.selections };
      const collapsedNodeIds = { ...state.collapsedNodeIds };
      delete selections[panelInstanceId];
      delete collapsedNodeIds[panelInstanceId];
      return { selections, collapsedNodeIds };
    }),
  reset: () => set({ selections: {}, collapsedNodeIds: {} }),
}));

export function getPaneSelection(
  panelInstanceId: EditorPanePanelId | undefined,
): EditorPaneSelection {
  if (!panelInstanceId) return EMPTY_EDITOR_PANE_SELECTION;
  return (
    useEditorPaneStateStore.getState().selections[panelInstanceId] ?? EMPTY_EDITOR_PANE_SELECTION
  );
}
