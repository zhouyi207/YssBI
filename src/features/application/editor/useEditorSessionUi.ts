import { useMemo } from "react";
import { useEditorStore, useEditorUIState, useEditorUIActions } from "@/features/core/editor";

export type EditorSessionUi = ReturnType<typeof useEditorUIState> &
  ReturnType<typeof useEditorUIActions>;

export function useEditorSessionUi(): EditorSessionUi {
  const uiState = useEditorUIState();
  const uiActions = useEditorUIActions();
  return useMemo(() => ({ ...uiState, ...uiActions }), [uiState, uiActions]);
}

export function useOptionalEditorSessionUi(enabled: boolean): EditorSessionUi {
  const uiActions = useEditorUIActions();
  const contextMenu = useEditorStore((state) => (enabled ? state.contextMenu : null));
  const detailFocus = useEditorStore((state) => (enabled ? state.detailFocus : null));
  return useMemo(
    () => ({ contextMenu, detailFocus, ...uiActions }),
    [contextMenu, detailFocus, uiActions],
  );
}
