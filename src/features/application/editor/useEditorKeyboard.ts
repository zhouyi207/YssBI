import { useCallback, useEffect, useRef } from 'react';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { getEditorGroupActiveTabId, useEditorTabStore } from '@/features/core/layout/editorTabStore';
import { resolveEditorTargetGroupId } from '@/features/core/layout/layoutTabQueries';
import { getViewport, editorViewportScope } from '@/features/core/viewport';
import { isAppModalOpen, useModifierKeyStore } from '@/features/core/keyboard';
import { DEFAULT_VIEWPORT } from '@/app/appConfig/default';
import { exitZenMode, isZenModeActive } from '@/features/core/layout/workbenchZenMode';
import { addGlobalEventListener } from '@/shared/utils/globalEvent';

interface UseEditorKeyboardProps {
  deleteSelected: () => void;
  undo: () => void;
  redo: () => void;
  copy: () => void;
  cut: () => void;
  paste: (pos?: { x: number; y: number }) => void;
  duplicateSelected?: () => void;
  saveGraph: () => void;
  saveGraphAs: () => void;
  importGraph: () => void;
  addEvent: (name?: string, options?: { openAfterCreate?: boolean }) => void;
  closeTab: (id: string) => void;
  setActiveTabId: (id: string | null, targetGroupId?: string) => void;
  splitEditorRight: (groupId: string) => void;
  toggleLogPanel?: () => void;
  toggleSidebar?: () => void;
  toggleDetail?: () => void;
  toggleZenMode?: () => void;
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
  duplicateSelected,
  saveGraph,
  saveGraphAs,
  importGraph,
  addEvent,
  closeTab,
  setActiveTabId,
  splitEditorRight,
  toggleLogPanel,
  toggleSidebar,
  toggleDetail,
  toggleZenMode,
}: UseEditorKeyboardProps) {
  const lastMousePosRef = useRef({ x: 0, y: 0 });
  const pendingCtrlKRef = useRef(false);
  const pendingCtrlKTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const getActiveCanvasLocalPoint = useCallback((clientX: number, clientY: number) => {
    const layoutStore = useLayoutStore.getState();
    const gid = resolveEditorTargetGroupId(undefined, layoutStore.nodes, layoutStore);
    const el = document.getElementById(`layout-node-${gid}`);
    if (!el) return { x: 0, y: 0 };
    const rect = el.getBoundingClientRect();
    const graphPath = getEditorGroupActiveTabId(gid);
    const currentCanvas = graphPath ? getViewport(editorViewportScope(gid, graphPath)) : DEFAULT_VIEWPORT;
    return {
      x: (clientX - rect.left - currentCanvas.x) / currentCanvas.scale,
      y: (clientY - rect.top - currentCanvas.y) / currentCanvas.scale,
    };
  }, []);

  useEffect(() => {
    const setModifierKeys = useModifierKeyStore.getState().setModifierKeys;
    const resetModifierKeys = useModifierKeyStore.getState().resetModifierKeys;

    const handlePointerMove = (e: PointerEvent) => {
      lastMousePosRef.current = { x: e.clientX, y: e.clientY };
    };

    const handleKeyDown = (e: KeyboardEvent) => {
      setModifierKeys({ altKey: e.altKey, ctrlKey: e.ctrlKey });

      if (isAppModalOpen()) {
        return;
      }

      if (e.key === 'Escape' && isZenModeActive()) {
        e.preventDefault();
        exitZenMode();
        return;
      }

      if (e.key === 'Alt') {
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
          (isControlKey && ["s", "z", "y", "n", "o", "w", "`"].includes(e.key.toLowerCase())) ||
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
      } else if (isControlKey && e.key.toLowerCase() === "d") {
        e.preventDefault();
        duplicateSelected?.();
      } else if (isControlKey && e.key.toLowerCase() === "s") {
        e.preventDefault();
        if (e.shiftKey) saveGraphAs();
        else saveGraph();
      } else if (isControlKey && e.key.toLowerCase() === "o") {
        e.preventDefault();
        importGraph();
      } else if (isControlKey && e.key.toLowerCase() === "n") {
        e.preventDefault();
        addEvent(undefined, { openAfterCreate: true });
      } else if (isControlKey && e.key.toLowerCase() === "w") {
        e.preventDefault();
        const layoutStore = useLayoutStore.getState();
        const gid = layoutStore.activeEditorGroupId;
        if (gid) {
          const activeTabId = getEditorGroupActiveTabId(gid);
          if (activeTabId) closeTab(activeTabId);
        }
      } else if (isControlKey && e.key === "Tab") {
        e.preventDefault();
        const gid = useLayoutStore.getState().activeEditorGroupId;
        if (gid) {
          const tabs = useEditorTabStore.getState().resolveGroupTabs(gid);
          const activeTabId = getEditorGroupActiveTabId(gid);
          if (tabs.length > 1 && activeTabId) {
            const currentIndex = tabs.findIndex((t) => t.id === activeTabId);
            const nextIndex = e.shiftKey
              ? (currentIndex - 1 + tabs.length) % tabs.length
              : (currentIndex + 1) % tabs.length;
            setActiveTabId(tabs[nextIndex].id);
          }
        }
      } else if (isControlKey && e.key === "\\") {
        e.preventDefault();
        const gid = useLayoutStore.getState().activeEditorGroupId;
        if (gid) splitEditorRight(gid);
      } else if (isControlKey && e.key.toLowerCase() === 'b') {
        e.preventDefault();
        toggleSidebar?.();
      } else if (isControlKey && e.key.toLowerCase() === 'i') {
        e.preventDefault();
        toggleDetail?.();
      } else if (isControlKey && e.key === "`") {
        e.preventDefault();
        toggleLogPanel?.();
      } else if (isControlKey && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        pendingCtrlKRef.current = true;
        if (pendingCtrlKTimerRef.current) clearTimeout(pendingCtrlKTimerRef.current);
        pendingCtrlKTimerRef.current = setTimeout(() => {
          pendingCtrlKRef.current = false;
          pendingCtrlKTimerRef.current = null;
        }, 2000);
      } else if (pendingCtrlKRef.current && e.key.toLowerCase() === 'z') {
        e.preventDefault();
        pendingCtrlKRef.current = false;
        if (pendingCtrlKTimerRef.current) {
          clearTimeout(pendingCtrlKTimerRef.current);
          pendingCtrlKTimerRef.current = null;
        }
        toggleZenMode?.();
      }
    };

    const handleKeyUp = (e: KeyboardEvent) => {
      setModifierKeys({ altKey: e.altKey, ctrlKey: e.ctrlKey });
      if (e.key === 'Alt') {
        useLayoutStore.getState().setAltPressed(false);
      }
    };

    const handleBlur = () => {
      resetModifierKeys();
      useLayoutStore.getState().setAltPressed(false);
    };

    const cleanupKeyDown = addGlobalEventListener(window, 'keydown', handleKeyDown, { capture: true });
    const cleanupKeyUp = addGlobalEventListener(window, 'keyup', handleKeyUp, { capture: true });
    const cleanupPointerMove = addGlobalEventListener(window, 'pointermove', handlePointerMove, { capture: true });
    const cleanupBlur = addGlobalEventListener(window, 'blur', handleBlur);

    return () => {
      cleanupKeyDown();
      cleanupKeyUp();
      cleanupPointerMove();
      cleanupBlur();
      if (pendingCtrlKTimerRef.current) clearTimeout(pendingCtrlKTimerRef.current);
    };
  }, [
    deleteSelected,
    undo,
    redo,
    copy,
    cut,
    paste,
    duplicateSelected,
    saveGraph,
    saveGraphAs,
    importGraph,
    addEvent,
    closeTab,
    setActiveTabId,
    splitEditorRight,
    toggleLogPanel,
    toggleSidebar,
    toggleDetail,
    toggleZenMode,
  ]);
}
