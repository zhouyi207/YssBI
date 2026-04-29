import { useCallback, useContext, useMemo } from 'react';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { GroupContext, useEditorGroupWorkspace } from '@/features/core/editor';
import { useEditor } from './useEditor';

export { GroupContext };

/**
 * useEditorGroup Hook
 *
 * Context-aware editor hook that scopes operations to a specific group.
 * When used within a GroupContext, it automatically scopes to that group.
 * Otherwise, it uses the globally active group.
 *
 * Sidebar/detail 等非 Canvas 组件自动禁用 canvas 交互，避免注册全局 pointer 监听器。
 */
export function useEditorGroup(options?: { withCanvasInteraction?: boolean }) {
  const currentGroupId = useContext(GroupContext);
  const overrideGroupId = currentGroupId || undefined;
  const withCanvasInteraction =
    options?.withCanvasInteraction ?? (overrideGroupId !== 'sidebar' && overrideGroupId !== 'detail');
  const editor = useEditor({ withCanvasInteraction });

  const { groupId, tabs, activeTabId, nodes, variables, selectedNodeIds } = useEditorGroupWorkspace(overrideGroupId);
  const setActiveGroup = useLayoutStore((s) => s.setActiveGroup);

  // Wrapped handlers that ensure the correct group is active
  const wrappedOnCanvasPointerDown = useCallback((e: React.PointerEvent) => {
    if (useLayoutStore.getState().activeGroupId !== groupId) {
      setActiveGroup(groupId);
    }
    editor.onCanvasPointerDown(e, groupId);
  }, [groupId, editor.onCanvasPointerDown, setActiveGroup]);

  const wrappedOnNodePointerDown = useCallback((nodeId: string, e: React.PointerEvent) => {
    if (useLayoutStore.getState().activeGroupId !== groupId) {
      setActiveGroup(groupId);
    }
    editor.onNodePointerDown(nodeId, e, groupId);
  }, [groupId, editor.onNodePointerDown, setActiveGroup]);

  const wrappedOnPinPointerDown = useCallback((pinId: string, e: React.PointerEvent) => {
    if (useLayoutStore.getState().activeGroupId !== groupId) {
      setActiveGroup(groupId);
    }
    editor.onPinPointerDown(pinId, e, groupId);
  }, [groupId, editor.onPinPointerDown, setActiveGroup]);

  const wrappedSetCanvas = useCallback((updater: any, targetGroupId?: string) => {
    if (useLayoutStore.getState().activeGroupId !== groupId) {
      setActiveGroup(groupId);
    }
    editor.setCanvas(updater, targetGroupId || groupId);
  }, [groupId, editor.setCanvas, setActiveGroup]);

  // Return combined object - memoize to prevent unnecessary re-renders
  return useMemo(() => ({
    ...editor,
    groupId,
    tabs,
    activeTabId,
    nodes,
    variables,
    selectedNodeIds,
    onCanvasPointerDown: wrappedOnCanvasPointerDown,
    onNodePointerDown: wrappedOnNodePointerDown,
    onPinPointerDown: wrappedOnPinPointerDown,
    setCanvas: wrappedSetCanvas,
  }), [
    editor,
    groupId,
    tabs,
    activeTabId,
    nodes,
    variables,
    selectedNodeIds,
    wrappedOnCanvasPointerDown,
    wrappedOnNodePointerDown,
    wrappedOnPinPointerDown,
    wrappedSetCanvas,
  ]);
}
