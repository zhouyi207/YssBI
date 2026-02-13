import { useCallback, useContext, createContext } from 'react';
import { useLayoutStore, LayoutState } from '@/features/layoutStore/layoutStore';
import { useTabNodes, useTabVariables } from '@/features/node-registry/stores/useNodeStore';
import { useShallow } from 'zustand/react/shallow';
import { useEditor } from './useEditor';

/**
 * GroupContext for scoped canvas operations
 * When a component is wrapped in GroupContext.Provider, useCanvasCompat will scope to that group
 */
export const GroupContext = createContext<string | null>(null);

/**
 * useCanvasCompat Hook
 * 
 * Compatibility hook that provides the same API as the old useCanvas hook.
 * This hook is context-aware: when used within a GroupContext, it automatically 
 * scopes operations to that group. Otherwise, it uses the globally active group.
 * 
 * @deprecated Consider using useEditor directly for new code
 */
export function useCanvasCompat() {
  const editor = useEditor();
  
  const currentGroupId = useContext(GroupContext);
  const activeGroupIdFromStore = useLayoutStore(useCallback((s: LayoutState) => s.activeGroupId, []));
  
  // If we are in a specific group context, use that ID. Otherwise fallback to the globally active one.
  const activeGroupId = currentGroupId || activeGroupIdFromStore || 'default_editor';

  // Resolve the group object for this context from layoutStore
  const nodeSelector = useCallback((s: LayoutState) => s.nodes[activeGroupId], [activeGroupId]);
  const node = useLayoutStore(useShallow(nodeSelector));
  
  // Core logic: If current Context points to a non-editor node (like Sidebar/Detail),
  // data logic (tabs/nodes/variables) should fall back to the currently active editor group.
  const activeEditorGroupId = useLayoutStore((s: LayoutState) => s.activeEditorGroupId);
  const isEditor = node?.type === 'component' && !!node.data?.tabs;
  
  const functionalNode = isEditor ? node : (useLayoutStore.getState().nodes[activeEditorGroupId || ''] || node);

  const tabs = functionalNode?.data?.tabs || [];
  const activeTabId = functionalNode?.data?.activeTabId || null;

  // Use the custom hook to efficiently retrieve nodes for the active tab
  const nodes = useTabNodes(activeTabId);
  const variables = useTabVariables(activeTabId);
  const selectedNodeIds = functionalNode?.data?.params?.selectedNodeIds || [];

  // Helper to activate this group when interaction starts
  const setActiveGroup = useLayoutStore(s => s.setActiveGroup);
  const ensureActive = () => {
    if (activeGroupIdFromStore !== activeGroupId) {
      setActiveGroup(activeGroupId);
    }
  };

  // Wrap interaction handlers to ensure the correct group is active
  const wrappedOnCanvasPointerDown = (e: React.PointerEvent) => {
    ensureActive();
    editor.onCanvasPointerDown(e, activeGroupId);
  };

  const wrappedOnNodePointerDown = (nodeId: string, e: React.PointerEvent) => {
    ensureActive();
    editor.onNodePointerDown(nodeId, e, activeGroupId);
  };

  const wrappedOnPinPointerDown = (pinId: string, e: React.PointerEvent) => {
    ensureActive();
    editor.onPinPointerDown(pinId, e, activeGroupId);
  };

  const wrappedOnCanvasWheel = (e: React.WheelEvent, targetGroupId?: string) => {
    ensureActive();
    editor.onCanvasWheel(e, targetGroupId || activeGroupId);
  };

  const wrappedSetCanvas = (updater: any, targetGroupId?: string) => {
    ensureActive();
    editor.setCanvas(updater, targetGroupId || activeGroupId);
  };

  // Merge global editor state with group-specific state
  return {
    ...editor,
    groupId: activeGroupId,
    tabs,
    activeTabId,
    // Override global state with localized state
    nodes,
    variables,
    selectedNodeIds,
    // Override handlers
    onCanvasPointerDown: wrappedOnCanvasPointerDown,
    onNodePointerDown: wrappedOnNodePointerDown,
    onPinPointerDown: wrappedOnPinPointerDown,
    onCanvasWheel: wrappedOnCanvasWheel,
    setCanvas: wrappedSetCanvas,
  };
}

/**
 * Alias for backward compatibility
 * @deprecated Use useCanvasCompat or useEditor directly
 */
export const useCanvas = useCanvasCompat;
