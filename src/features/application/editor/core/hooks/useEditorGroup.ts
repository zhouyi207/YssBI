import { useCallback, useContext, createContext } from 'react';
import { useLayoutStore, LayoutState } from '@/features/core/layout/layoutStore';
import { useTabNodes, useTabVariables } from '@/features/core/_node/useNodeStore';
import { useEditor } from './useEditor';

/**
 * GroupContext for scoped canvas operations
 * When a component is wrapped in GroupContext.Provider, operations will scope to that group
 */
export const GroupContext = createContext<string | null>(null);

/**
 * useEditorGroup Hook
 * 
 * Context-aware editor hook that scopes operations to a specific group.
 * When used within a GroupContext, it automatically scopes to that group.
 * Otherwise, it uses the globally active group.
 */
export function useEditorGroup() {
  const editor = useEditor();
  const currentGroupId = useContext(GroupContext);
  
  // Get the active group ID
  const activeGroupIdFromStore = useLayoutStore((s: LayoutState) => s.activeGroupId);
  const groupId = currentGroupId || activeGroupIdFromStore || 'default_editor';

  // Get node and activeEditorGroupId
  const node = useLayoutStore(useCallback((s: LayoutState) => s.nodes[groupId], [groupId]));
  const activeEditorGroupId = useLayoutStore((s: LayoutState) => s.activeEditorGroupId);
  const setActiveGroup = useLayoutStore(s => s.setActiveGroup);

  // Determine functional node (editor or fallback)
  const isEditor = node?.type === 'component' && !!node.data?.tabs;
  const functionalNode = isEditor ? node : (useLayoutStore.getState().nodes[activeEditorGroupId || ''] || node);

  const tabs = functionalNode?.data?.tabs || [];
  const activeTabId = functionalNode?.data?.activeTabId || null;
  const selectedNodeIds = functionalNode?.data?.params?.selectedNodeIds || [];

  // Efficiently retrieve nodes and variables for the active tab
  const nodes = useTabNodes(activeTabId);
  const variables = useTabVariables(activeTabId);

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

  const wrappedOnCanvasWheel = useCallback((e: React.WheelEvent, targetGroupId?: string) => {
    if (useLayoutStore.getState().activeGroupId !== groupId) {
      setActiveGroup(groupId);
    }
    editor.onCanvasWheel(e, targetGroupId || groupId);
  }, [groupId, editor.onCanvasWheel, setActiveGroup]);

  const wrappedSetCanvas = useCallback((updater: any, targetGroupId?: string) => {
    if (useLayoutStore.getState().activeGroupId !== groupId) {
      setActiveGroup(groupId);
    }
    editor.setCanvas(updater, targetGroupId || groupId);
  }, [groupId, editor.setCanvas, setActiveGroup]);

  // Return combined object
  return {
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
    onCanvasWheel: wrappedOnCanvasWheel,
    setCanvas: wrappedSetCanvas,
  };
}
