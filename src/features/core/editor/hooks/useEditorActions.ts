/**
 * 编辑器操作（组合 hook）
 * 组合 useEditorCanvasActions、useEditorUIActions
 * 并提供 refs 供 canvas pointer loop 使用（viewportRef 为 EditorViewport 快照）
 */
import { useRef, useEffect } from 'react';
import { getViewport, subscribeToViewport } from '@/features/core/viewport';
import { useActiveEditorGroup } from './useActiveEditorGroup';
import { useEditorCanvasActions } from './useEditorCanvasActions';
import { useEditorUIActions } from './useEditorUIActions';

type ActiveEditorGroup = ReturnType<typeof useActiveEditorGroup>;

export function useEditorActions(active: ActiveEditorGroup) {
  const editorGroupId = active.focusedEditorGroupId ?? active.groupId;
  const activeGroupIdRef = useRef(editorGroupId);
  const activeTabIdRef = useRef(active.activeTabId);
  activeGroupIdRef.current = editorGroupId;
  activeTabIdRef.current = active.activeTabId;

  const canvasActions = useEditorCanvasActions(activeTabIdRef);
  const uiActions = useEditorUIActions();

  const viewportRef = useRef(getViewport(active.activeTabId ?? ''));

  useEffect(() => {
    const graphPath = activeTabIdRef.current;
    if (!graphPath) return;
    viewportRef.current = getViewport(graphPath);
    return subscribeToViewport(graphPath, (viewport) => {
      viewportRef.current = viewport;
    });
  }, [active.activeTabId]);

  return {
    activeGroupIdRef,
    activeTabIdRef,
    viewportRef,
    ...canvasActions,
    ...uiActions,
  };
}
