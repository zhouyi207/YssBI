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
  const activeGroupIdRef = useRef(active.activeGroupId);
  const activeTabIdRef = useRef(active.activeTabId);
  activeGroupIdRef.current = active.activeGroupId;
  activeTabIdRef.current = active.activeTabId;
  const nodeActions = useEditorNodeActions(activeTabIdRef, active.activeGroupId);
  const canvasActions = useEditorCanvasActions(active.activeGroupId);
  const uiActions = useEditorUIActions();
  const layoutActions = useEditorLayoutActions();

  const canvasRef = useRef(useViewportStore.getState().viewports[active.activeGroupId] || DEFAULT_VIEWPORT);

  useEffect(() => {
    const unsub = useViewportStore.subscribe((state) => {
      const currentGroupId = useLayoutStore.getState().activeGroupId;
      if (currentGroupId && state.viewports[currentGroupId]) {
        canvasRef.current = state.viewports[currentGroupId];
      }
    });
    const current = useViewportStore.getState().viewports[useLayoutStore.getState().activeGroupId || ''];
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
