/**
 * 编辑器操作（组合 hook）
 * 组合 useEditorCanvasActions、useEditorUIActions
 * 并提供 refs 供 canvas pointer loop 使用（viewportRef 为 EditorViewport 快照）
 */
import { useRef, useEffect } from 'react';
import { DEFAULT_VIEWPORT } from '@/app/appConfig/default';
import { getViewport, subscribeToViewport, editorViewportScope } from '@/features/core/viewport';
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

  const canvasActions = useEditorCanvasActions(activeGroupIdRef, activeTabIdRef);
  const uiActions = useEditorUIActions();

  const viewportScope =
    editorGroupId && active.activeTabId
      ? editorViewportScope(editorGroupId, active.activeTabId)
      : null;

  const viewportRef = useRef(viewportScope ? getViewport(viewportScope) : DEFAULT_VIEWPORT);

  useEffect(() => {
    if (!viewportScope) return;
    viewportRef.current = getViewport(viewportScope);
    return subscribeToViewport(viewportScope, (viewport) => {
      viewportRef.current = viewport;
    });
  }, [viewportScope?.groupId, viewportScope?.graphPath]);

  return {
    activeGroupIdRef,
    activeTabIdRef,
    viewportRef,
    ...canvasActions,
    ...uiActions,
  };
}
