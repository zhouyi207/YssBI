import { create } from "zustand";
import { shallow } from "zustand/shallow";
import { produce } from "immer";

type EditorPanePanelId = string;

export interface EditorPaneSelection {
  selectedNodeIds: string[];
  selectedConnectionIds: string[];
}

export interface DatabasePaneView {
  databaseId: string;
  selectedCellText: string;
}

export const EMPTY_EDITOR_PANE_SELECTION: EditorPaneSelection = {
  selectedNodeIds: [],
  selectedConnectionIds: [],
};

interface EditorPaneState {
  selections: Record<EditorPanePanelId, EditorPaneSelection>;
  collapsedNodeIds: Record<EditorPanePanelId, string[]>;
  databaseViews: Record<EditorPanePanelId, DatabasePaneView>;
  setDatabaseView(panelInstanceId: EditorPanePanelId, view: DatabasePaneView | undefined): void;
  setSelectedNodeIds(panelInstanceId: EditorPanePanelId, ids: string[]): void;
  setSelectedConnectionIds(panelInstanceId: EditorPanePanelId, ids: string[]): void;
  setNodeCollapsed(panelInstanceId: EditorPanePanelId, nodeId: string, collapsed: boolean): void;
  clearSelection(panelInstanceId: EditorPanePanelId): void;
  release(panelInstanceId: EditorPanePanelId): void;
  reset(): void;
}

const unique = (ids: readonly string[]): string[] => [...new Set(ids)];

/** Pane-local UI state only; FlexLayout remains authoritative for panel placement. */
export const useEditorPaneStateStore = create<EditorPaneState>((set) => {
  const setSelection = (panelInstanceId: EditorPanePanelId, selection: EditorPaneSelection) =>
    set(
      produce((state: EditorPaneState) => {
        const current = state.selections[panelInstanceId] ?? EMPTY_EDITOR_PANE_SELECTION;
        if (
          shallow(current.selectedNodeIds, selection.selectedNodeIds) &&
          shallow(current.selectedConnectionIds, selection.selectedConnectionIds)
        )
          return;
        state.selections[panelInstanceId] = selection;
      }),
    );
  return {
    selections: {},
    collapsedNodeIds: {},
    databaseViews: {},
    setDatabaseView: (panelInstanceId, view) =>
      set(
        produce((state: EditorPaneState) => {
          if (shallow(state.databaseViews[panelInstanceId], view)) return;
          if (view) state.databaseViews[panelInstanceId] = view;
          else delete state.databaseViews[panelInstanceId];
        }),
      ),
    setSelectedNodeIds: (panelInstanceId, ids) =>
      setSelection(panelInstanceId, { selectedNodeIds: unique(ids), selectedConnectionIds: [] }),
    setSelectedConnectionIds: (panelInstanceId, ids) =>
      setSelection(panelInstanceId, { selectedNodeIds: [], selectedConnectionIds: unique(ids) }),
    setNodeCollapsed: (panelInstanceId, nodeId, collapsed) =>
      set(
        produce((state: EditorPaneState) => {
          const ids = state.collapsedNodeIds[panelInstanceId] ?? [];
          if (ids.includes(nodeId) === collapsed) return;
          state.collapsedNodeIds[panelInstanceId] = collapsed
            ? [...ids, nodeId]
            : ids.filter((id) => id !== nodeId);
        }),
      ),
    clearSelection: (panelInstanceId) => setSelection(panelInstanceId, EMPTY_EDITOR_PANE_SELECTION),
    release: (panelInstanceId) =>
      set(
        produce((state: EditorPaneState) => {
          delete state.selections[panelInstanceId];
          delete state.collapsedNodeIds[panelInstanceId];
          delete state.databaseViews[panelInstanceId];
        }),
      ),
    reset: () =>
      set(
        produce((state: EditorPaneState) => {
          if (Object.keys(state.selections).length) state.selections = {};
          if (Object.keys(state.collapsedNodeIds).length) state.collapsedNodeIds = {};
          if (Object.keys(state.databaseViews).length) state.databaseViews = {};
        }),
      ),
  };
});

export function getPaneSelection(
  panelInstanceId: EditorPanePanelId | undefined,
): EditorPaneSelection {
  if (!panelInstanceId) return EMPTY_EDITOR_PANE_SELECTION;
  return (
    useEditorPaneStateStore.getState().selections[panelInstanceId] ?? EMPTY_EDITOR_PANE_SELECTION
  );
}
