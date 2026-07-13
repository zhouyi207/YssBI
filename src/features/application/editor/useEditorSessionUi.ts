import { useMemo } from 'react';
import { useEditorStore, useEditorUIState, useEditorUIActions } from '@/features/core/editor';

export type EditorSessionUi = ReturnType<typeof useEditorUIState> &
  ReturnType<typeof useEditorUIActions>;

/** Transient canvas / detail UI. */
export function useEditorSessionUi(): EditorSessionUi {
  const uiState = useEditorUIState();
  const uiActions = useEditorUIActions();

  return useMemo(
    () => ({
      ...uiState,
      ...uiActions,
    }),
    [uiState, uiActions],
  );
}

/**
 * Canvas UI with gated subscriptions — preview canvases skip re-renders when
 * context menu / pending connection changes on the active group.
 */
export function useOptionalEditorSessionUi(enabled: boolean): EditorSessionUi {
  const uiActions = useEditorUIActions();

  const contextMenu = useEditorStore((s) => (enabled ? s.contextMenu : null));
  const detailFocus = useEditorStore((s) => (enabled ? s.detailFocus : null));
  const pendingConnection = useEditorStore((s) => (enabled ? s.pendingConnection : null));

  return useMemo(
    (): EditorSessionUi => ({
      contextMenu,
      detailFocus,
      pendingConnection,
      ...uiActions,
    }),
    [contextMenu, detailFocus, pendingConnection, uiActions],
  );
}
