/**
 * 编辑器操作（组合 hook）
 * 组合 useEditorNodeActions、useEditorCanvasActions、useEditorUIActions、useEditorLayoutActions
 * 并提供 refs 供 canvas pointer loop 使用（viewportRef 为 GraphPosition 快照）
 */
import { useRef, useEffect } from 'react';
import { getViewport, subscribeToViewport } from '@/features/core/viewport';
import { useActiveEditorGroup } from './useActiveEditorGroup';
import { useEditorNodeActions } from './useEditorNodeActions';
import { useEditorCanvasActions } from './useEditorCanvasActions';
import { useEditorUIActions } from './useEditorUIActions';
import { useEditorLayoutActions } from './useEditorLayoutActions';

type ActiveEditorGroup = ReturnType<typeof useActiveEditorGroup>;

export function useEditorActions(active: ActiveEditorGroup) {
  const editorGroupId = active.activeEditorGroupId;
  const activeGroupIdRef = useRef(editorGroupId);
  const activeTabIdRef = useRef(active.activeTabId);
  activeGroupIdRef.current = editorGroupId;
  activeTabIdRef.current = active.activeTabId;

  const nodeActions = useEditorNodeActions(activeTabIdRef, editorGroupId);
  const canvasActions = useEditorCanvasActions(activeTabIdRef);
  const uiActions = useEditorUIActions();
  const layoutActions = useEditorLayoutActions();

  const viewportRef = useRef(getViewport(active.activeTabId ?? ''));

  useEffect(() => {
    const graphId = activeTabIdRef.current;
    if (!graphId) return;
    viewportRef.current = getViewport(graphId);
    return subscribeToViewport(graphId, (viewport) => {
      viewportRef.current = viewport;
    });
  }, [active.activeTabId]);

  return {
    activeGroupIdRef,
    activeTabIdRef,
    viewportRef,
    ...nodeActions,
    ...canvasActions,
    ...uiActions,
    ...layoutActions,
  };
}
