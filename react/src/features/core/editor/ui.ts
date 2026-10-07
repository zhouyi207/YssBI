import { createReadProjection, useReadProjection } from "@/features/core/state/readProjection";

import type { DeepReadonly } from "@/shared/types/deepReadonly";
import { useEditorStore, type EditorContextMenuState } from "./stores/useEditorStore";
import type { DetailFocus } from "@/features/core/editor/detail/detailTypes";

export interface EditorUiSnapshot {
  readonly contextMenu: DeepReadonly<EditorContextMenuState> | null;
  readonly detailFocus: DeepReadonly<DetailFocus> | null;
}

export interface EditorUiCapability {
  readonly getSnapshot: () => DeepReadonly<EditorUiSnapshot>;
  readonly subscribe: (listener: () => void) => () => void;
  readonly setContextMenu: (menu: DeepReadonly<EditorContextMenuState> | null) => void;
  readonly setDetailFocus: (focus: DeepReadonly<DetailFocus>) => void;
  readonly clearDetailFocus: () => void;
}

function buildSnapshot(): DeepReadonly<EditorUiSnapshot> {
  const state = useEditorStore.getState();
  return {
    contextMenu: state.contextMenu,
    detailFocus: state.detailFocus,
  };
}

const projection = createReadProjection(buildSnapshot, [useEditorStore]);
export const getEditorUiSnapshot = projection.getSnapshot;
export const subscribeEditorUi = projection.subscribe;
export function useEditorUi<T>(selector: (snapshot: DeepReadonly<EditorUiSnapshot>) => T): T {
  return useReadProjection(projection, selector);
}

export const editorUi: EditorUiCapability = {
  getSnapshot: getEditorUiSnapshot,
  subscribe: subscribeEditorUi,
  setContextMenu: (menu) => useEditorStore.getState().setContextMenu(menu ? { ...menu } : null),
  setDetailFocus: (focus) => useEditorStore.getState().setDetailFocus(focus as DetailFocus),
  clearDetailFocus: () => useEditorStore.getState().clearDetailFocus(),
};
