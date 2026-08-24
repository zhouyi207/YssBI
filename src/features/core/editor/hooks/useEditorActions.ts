/**
 * 编辑器操作（组合 hook）
 * 组合 useEditorCanvasActions、useEditorUIActions
 * 并提供 refs 供 canvas pointer loop 使用（viewportRef 为 EditorViewport 快照）
 */
import { useCallback, useEffect, useRef } from 'react';
import { DEFAULT_VIEWPORT } from '@/app/appConfig/default';
import {
  commitViewport,
  editorViewportScope,
  getViewport,
  setViewportLive,
  subscribeToViewport,
  type EditorViewport,
} from '@/features/core/viewport';
import { useActiveEditorGroup } from './useActiveEditorGroup';
import { useEditorUIActions } from './useEditorUIActions';

type ActiveEditorGroup = ReturnType<typeof useActiveEditorGroup>;

export function useEditorActions(active: ActiveEditorGroup) {
  const editorGroupId = active.groupId;
  const activeGroupIdRef = useRef<string | null>(editorGroupId);
  const activeTabIdRef = useRef<string | null>(active.activeTabId);
  activeGroupIdRef.current = editorGroupId;
  activeTabIdRef.current = active.activeTabId;

  const setCanvas = useCallback((
    updater: EditorViewport | ((previous: EditorViewport) => EditorViewport),
    targetGraphPath?: string,
  ) => {
    const groupId = activeGroupIdRef.current;
    const graphPath = targetGraphPath ?? activeTabIdRef.current;
    if (!groupId || !graphPath) return;
    const scope = editorViewportScope(groupId, graphPath);
    setViewportLive(scope, updater);
    commitViewport(scope);
  }, []);
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
    setCanvas,
    ...uiActions,
  };
}
