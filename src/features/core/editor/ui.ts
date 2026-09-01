import { useSyncExternalStore } from "react";

import type { DeepReadonly } from "@/shared/types/deepReadonly";
import { useEditorStore, type EditorContextMenuState } from "./stores/useEditorStore";
import type { DetailFocus } from "@/features/core/editor/detail/detailTypes";

export interface EditorUiSnapshot {
  readonly contextMenu: DeepReadonly<EditorContextMenuState> | null;
  readonly detailFocus: DeepReadonly<DetailFocus> | null;
  readonly variablesGraphScopePath: string | null;
}

export interface EditorUiCapability {
  readonly getSnapshot: () => DeepReadonly<EditorUiSnapshot>;
  readonly subscribe: (listener: () => void) => () => void;
  readonly setContextMenu: (menu: DeepReadonly<EditorContextMenuState> | null) => void;
  readonly setDetailFocus: (focus: DeepReadonly<DetailFocus>) => void;
  readonly clearDetailFocus: () => void;
  readonly setVariablesGraphScope: (graphPath: string | null) => void;
}

function buildSnapshot(): DeepReadonly<EditorUiSnapshot> {
  const state = useEditorStore.getState();
  return Object.freeze({
    contextMenu: state.contextMenu ? Object.freeze({ ...state.contextMenu }) : null,
    detailFocus: state.detailFocus,
    variablesGraphScopePath: state.variablesGraphScopePath,
  });
}

let currentSnapshot = buildSnapshot();
const listeners = new Set<() => void>();

function refreshSnapshot(): void {
  currentSnapshot = buildSnapshot();
  for (const listener of listeners) listener();
}

useEditorStore.subscribe(refreshSnapshot);

export function getEditorUiSnapshot(): DeepReadonly<EditorUiSnapshot> {
  return currentSnapshot;
}

export function subscribeEditorUi(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useEditorUi<T>(selector: (snapshot: DeepReadonly<EditorUiSnapshot>) => T): T {
  const snapshot = useSyncExternalStore(
    subscribeEditorUi,
    getEditorUiSnapshot,
    getEditorUiSnapshot,
  );
  return selector(snapshot);
}

export const editorUi: EditorUiCapability = {
  getSnapshot: getEditorUiSnapshot,
  subscribe: subscribeEditorUi,
  setContextMenu: (menu) => useEditorStore.getState().setContextMenu(menu ? { ...menu } : null),
  setDetailFocus: (focus) => useEditorStore.getState().setDetailFocus(focus as DetailFocus),
  clearDetailFocus: () => useEditorStore.getState().clearDetailFocus(),
  setVariablesGraphScope: (graphPath) =>
    useEditorStore.getState().setVariablesGraphScope(graphPath),
};
