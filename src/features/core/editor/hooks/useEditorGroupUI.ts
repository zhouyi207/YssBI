/**
 * 编辑器组 UI 状态和操作：contextMenu、pendingConnection、setters
 * 直接使用 core hooks，无 application 依赖
 */
import { useMemo } from 'react';
import { useEditorUIState } from './useEditorUIState';
import { useEditorUIActions } from './useEditorUIActions';

export function useEditorGroupUI() {
  const uiState = useEditorUIState();
  const uiActions = useEditorUIActions();

  return useMemo(
    () => ({
      ...uiState,
      ...uiActions,
    }),
    [uiState, uiActions]
  );
}
