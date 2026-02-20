/**
 * 编辑器操作（组合 hook）
 * 组合 useEditorNodeActions、useEditorCanvasActions、useEditorUIActions、useEditorLayoutActions
 * 并提供 refs 供 canvas interaction 使用
 */
import { useRef, useEffect } from 'react';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { useViewportStore } from '@/features/core/viewport';
import { DEFAULT_VIEWPORT } from '@/app/appConfig/default';
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

  const canvasRef = useRef(useViewportStore.getState().viewports[editorGroupId] || DEFAULT_VIEWPORT);

  useEffect(() => {
    const unsub = useViewportStore.subscribe((state) => {
      const editorGid = useLayoutStore.getState().activeEditorGroupId;
      if (editorGid && state.viewports[editorGid]) {
        canvasRef.current = state.viewports[editorGid];
      }
    });
    const editorGid = useLayoutStore.getState().activeEditorGroupId || '';
    const current = useViewportStore.getState().viewports[editorGid];
    if (current) canvasRef.current = current;
    return unsub;
  }, []);

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
