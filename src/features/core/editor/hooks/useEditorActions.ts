/**
 * 编辑器操作（组合 hook）
 * 组合 useEditorNodeActions、useEditorCanvasActions、useEditorUIActions、useEditorLayoutActions
 * 并提供 refs 供 canvas interaction 使用
 */
import { useRef, useEffect } from 'react';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { getViewport, subscribeToViewport } from '@/features/core/viewport';
import { useActiveEditorGroup } from './useActiveEditorGroup';
import { useEditorNodeActions } from './useEditorNodeActions';
import { useEditorCanvasActions } from './useEditorCanvasActions';
import { useEditorUIActions } from './useEditorUIActions';
import { useEditorLayoutActions } from './useEditorLayoutActions';

export function useEditorActions(overrideGroupId?: string | null) {
  const active = useActiveEditorGroup(overrideGroupId);
  const editorGroupId = active.activeEditorGroupId;
  const activeGroupIdRef = useRef(editorGroupId);
  const activeTabIdRef = useRef(active.activeTabId);
  activeGroupIdRef.current = editorGroupId;
  activeTabIdRef.current = active.activeTabId;
  const nodeActions = useEditorNodeActions(activeTabIdRef, editorGroupId);
  const canvasActions = useEditorCanvasActions(editorGroupId);
  const uiActions = useEditorUIActions();
  const layoutActions = useEditorLayoutActions();

  const canvasRef = useRef(getViewport(editorGroupId));

  useEffect(() => {
    const editorGid = useLayoutStore.getState().activeEditorGroupId || editorGroupId;
    canvasRef.current = getViewport(editorGid);
    return subscribeToViewport(editorGid, (viewport) => {
      canvasRef.current = viewport;
    });
  }, [editorGroupId]);

  return {
    activeGroupIdRef,
    activeTabIdRef,
    canvasRef,
    ...nodeActions,
    ...canvasActions,
    ...uiActions,
    ...layoutActions,
  };
}
