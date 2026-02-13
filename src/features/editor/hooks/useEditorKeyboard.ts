import { useEffect, useRef } from 'react';
import { useLayoutStore, LayoutState } from '@/features/layoutStore/layoutStore';

interface UseEditorKeyboardProps {
  deleteSelected: () => void;
  undo: () => void;
  redo: () => void;
  copy: () => void;
  cut: () => void;
  paste: (pos?: { x: number; y: number }) => void;
  saveGraph: () => void;
  saveGraphAs: () => void;
  importGraph: () => void;
  addEvent: () => void;
  closeTab: (id: string) => void;
  setActiveTabId: (id: string | null, targetGroupId?: string) => void;
  splitEditorRight: (groupId: string) => void;
  getActiveCanvasLocalPoint: (clientX: number, clientY: number) => { x: number; y: number };
}

/**
 * Editor Keyboard Hook
 * Handles all keyboard shortcuts for the editor
 */
export function useEditorKeyboard({
  deleteSelected,
  undo,
  redo,
  copy,
  cut,
  paste,
  saveGraph,
  saveGraphAs,
  importGraph,
  addEvent,
  closeTab,
  setActiveTabId,
  splitEditorRight,
  getActiveCanvasLocalPoint,
}: UseEditorKeyboardProps) {
  const lastMousePosRef = useRef({ x: 0, y: 0 });

  useEffect(() => {
    const handlePointerMove = (e: PointerEvent) => {
      lastMousePosRef.current = { x: e.clientX, y: e.clientY };
    };

    const handleKeyDown = (e: KeyboardEvent) => {
      // Track modifier keys globally
      (window as any)._lastAltKey = e.altKey;
      (window as any)._lastCtrlKey = e.ctrlKey;

      if (e.key === 'Alt') {
        e.preventDefault();
        if (e.repeat) return;
        useLayoutStore.getState().setAltPressed(true);
      }

      // Check if we're in an input field
      const isInput =
        document.activeElement?.tagName === "INPUT" ||
        document.activeElement?.tagName === "TEXTAREA" ||
        (document.activeElement as HTMLElement)?.isContentEditable;

      const isControlKey = e.ctrlKey || e.metaKey;

      if (isInput) {
        // Only allow specific global shortcuts in input fields
        const allowedInInput =
          (isControlKey && ["s", "z", "y", "n", "o", "w"].includes(e.key.toLowerCase())) ||
          (isControlKey && e.key === "Tab");

        if (!allowedInInput) return;
      }

      // Keyboard shortcuts
      if (e.key === "Delete" || e.key === "Backspace") {
        deleteSelected();
      } else if (isControlKey && e.key.toLowerCase() === "z") {
        if (e.shiftKey) redo();
        else undo();
      } else if (isControlKey && e.key.toLowerCase() === "y") {
        redo();
      } else if (isControlKey && e.key.toLowerCase() === "c") {
        copy();
      } else if (isControlKey && e.key.toLowerCase() === "x") {
        cut();
      } else if (isControlKey && e.key.toLowerCase() === "v") {
        paste(getActiveCanvasLocalPoint(lastMousePosRef.current.x, lastMousePosRef.current.y));
      } else if (isControlKey && e.key.toLowerCase() === "s") {
        e.preventDefault();
        if (e.shiftKey) saveGraphAs();
        else saveGraph();
      } else if (isControlKey && e.key.toLowerCase() === "o") {
        e.preventDefault();
        importGraph();
      } else if (isControlKey && e.key.toLowerCase() === "n") {
        e.preventDefault();
        addEvent();
      } else if (isControlKey && e.key.toLowerCase() === "w") {
        e.preventDefault();
        const layoutStore = useLayoutStore.getState();
        const gid = layoutStore.activeEditorGroupId || layoutStore.activeGroupId;
        if (gid) {
          const node = layoutStore.nodes[gid];
          const activeTabId = node?.data?.activeTabId;
          if (activeTabId) closeTab(activeTabId);
        }
      } else if (isControlKey && e.key === "Tab") {
        e.preventDefault();
        const gid = useLayoutStore.getState().activeEditorGroupId || useLayoutStore.getState().activeGroupId;
        if (gid) {
          const node = useLayoutStore.getState().nodes[gid];
          const tabs = node?.data?.tabs || [];
          const activeTabId = node?.data?.activeTabId;
          if (tabs.length > 1) {
            const currentIndex = tabs.findIndex(t => t.id === activeTabId);
            const nextIndex = e.shiftKey
              ? (currentIndex - 1 + tabs.length) % tabs.length
              : (currentIndex + 1) % tabs.length;
            setActiveTabId(tabs[nextIndex].id);
          }
        }
      } else if (isControlKey && e.key === "\\") {
        e.preventDefault();
        const gid = useLayoutStore.getState().activeEditorGroupId || useLayoutStore.getState().activeGroupId;
        if (gid) splitEditorRight(gid);
      }
    };

    const handleKeyUp = (e: KeyboardEvent) => {
      (window as any)._lastAltKey = e.altKey;
      (window as any)._lastCtrlKey = e.ctrlKey;
      if (e.key === 'Alt') {
        useLayoutStore.getState().setAltPressed(false);
      }
    };

    const handleBlur = () => {
      useLayoutStore.getState().setAltPressed(false);
    };

    window.addEventListener('keydown', handleKeyDown, { capture: true });
    window.addEventListener('keyup', handleKeyUp, { capture: true });
    window.addEventListener('pointermove', handlePointerMove, { capture: true });
    window.addEventListener('blur', handleBlur);

    return () => {
      window.removeEventListener('keydown', handleKeyDown, { capture: true });
      window.removeEventListener('keyup', handleKeyUp, { capture: true });
      window.removeEventListener('pointermove', handlePointerMove, { capture: true });
      window.removeEventListener('blur', handleBlur);
    };
  }, [
    deleteSelected,
    undo,
    redo,
    copy,
    cut,
    paste,
    saveGraph,
    saveGraphAs,
    importGraph,
    addEvent,
    closeTab,
    setActiveTabId,
    splitEditorRight,
    getActiveCanvasLocalPoint,
  ]);
}
