import { useEffect, useRef } from 'react';
import { editorDockviewPort } from '@/features/core/dockview';
import {
  clearEditorGroupGraphSelection,
  getActiveLayoutTab,
  getEditorGroupGraphSelection,
  resolveEditorTargetGroupId,
} from '@/features/core/layout/layoutTabQueries';
import { getViewport, editorViewportScope } from '@/features/core/viewport';
import { isAppModalOpen, useModifierKeyStore } from '@/features/core/keyboard';
import { DEFAULT_VIEWPORT } from '@/app/appConfig/default';
import {
  exitZenMode,
  isZenModeActive,
  toggleZenMode,
} from '@/features/core/layout/workbenchZenMode';
import {
  toggleDetailVisibility,
  togglePanelCollapsed,
  toggleSidebarVisibility,
} from '@/features/core/layout/workbenchLayoutService';
import { useWorkbenchStore } from '@/features/core/workbench';
import { addGlobalEventListener } from '@/shared/utils/globalEvent';
import { useHistoryStore } from '@/features/core/history';
import { getCanvasInteraction, useGraphInteractionStore } from '@/features/core/graphInteraction/graphInteractionStore';
import { cancelCanvasInteraction } from '@/features/core/canvas/canvasInteractionCleanup';
import { useEditorStore } from '@/features/core/editor';
import { EDITOR_MUTATION_CAPABILITIES } from './editorMutationAvailability';
import { listDockviewGroupTabs } from './dockviewTabProjection';
import { useEditorSessionCommandsContext } from './EditorSessionContext';

function getActiveCanvasLocalPoint(clientX: number, clientY: number) {
  const groupId = resolveEditorTargetGroupId();
  const element = document.querySelector(`[data-editor-group-id="${groupId}"]`);
  if (!(element instanceof HTMLElement)) return { x: 0, y: 0 };
  const rect = element.getBoundingClientRect();
  const graphPath = getActiveLayoutTab(groupId)?.activeTabId;
  const viewport = graphPath
    ? getViewport(editorViewportScope(groupId, graphPath))
    : DEFAULT_VIEWPORT;
  return {
    x: (clientX - rect.left - viewport.x) / viewport.scale,
    y: (clientY - rect.top - viewport.y) / viewport.scale,
  };
}

/** Mounts the editor window's single ordered global keyboard shortcut listener set. */
export function useEditorKeyboard(): void {
  const commands = useEditorSessionCommandsContext();
  const lastMousePosRef = useRef({ x: 0, y: 0 });
  const pendingCtrlKRef = useRef(false);
  const pendingCtrlKTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    const setModifierKeys = useModifierKeyStore.getState().setModifierKeys;
    const resetModifierKeys = useModifierKeyStore.getState().resetModifierKeys;

    const handlePointerMove = (e: PointerEvent) => {
      lastMousePosRef.current = { x: e.clientX, y: e.clientY };
    };

    const handleKeyDown = (e: KeyboardEvent) => {
      setModifierKeys({ altKey: e.altKey, ctrlKey: e.ctrlKey, shiftKey: e.shiftKey });

      if (isAppModalOpen()) {
        return;
      }

      if (e.key === 'Escape') {
        const groupId = editorDockviewPort.getActiveGroupId();
        const graphPath = groupId ? getActiveLayoutTab(groupId)?.activeTabId : null;
        if (graphPath && groupId) {
          const interaction = getCanvasInteraction(useGraphInteractionStore.getState(), graphPath, groupId);
          if (interaction.type !== 'idle') {
            e.preventDefault();
            cancelCanvasInteraction(graphPath, groupId);
            if (interaction.type === 'pendingNodeCreation') {
              useEditorStore.getState().setContextMenu(null);
            }
            return;
          }
        }
        if (groupId) {
          const selection = getEditorGroupGraphSelection(groupId);
          if (selection.connectionIds.size > 0 || selection.nodeIds.size > 0) {
            e.preventDefault();
            clearEditorGroupGraphSelection(groupId);
            return;
          }
        }
        if (isZenModeActive()) {
          e.preventDefault();
          exitZenMode();
          return;
        }
      }

      if (e.key === 'F1') {
        e.preventDefault();
        useWorkbenchStore.getState().setNodeDocumentationOpen(true);
        return;
      }

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
      if (!e.repeat && isControlKey && e.key.toLowerCase() === 'a') {
        if (commands.selectAllNodes()) e.preventDefault();
      } else if (
        !e.repeat && !isControlKey && !e.altKey && !e.shiftKey && e.key.toLowerCase() === 'f'
      ) {
        if (commands.focusSelectedNodes()) e.preventDefault();
      } else if (!e.repeat && !isControlKey && !e.altKey && !e.shiftKey && e.key === 'Home') {
        if (commands.fitCompleteGraph()) e.preventDefault();
      } else if (e.key === "Delete" || e.key === "Backspace") {
        commands.deleteSelected();
      } else if (isControlKey && e.key.toLowerCase() === "z") {
        const { canUndo, canRedo, pending } = useHistoryStore.getState();
        if (e.shiftKey) {
          if (canRedo && !pending) commands.redo();
        } else if (canUndo && !pending) commands.undo();
      } else if (isControlKey && e.key.toLowerCase() === "y") {
        const { canRedo, pending } = useHistoryStore.getState();
        if (canRedo && !pending) commands.redo();
      } else if (isControlKey && e.key.toLowerCase() === "c") {
        e.preventDefault();
        if (!e.repeat) commands.copy();
      } else if (isControlKey && e.key.toLowerCase() === "x") {
        e.preventDefault();
        if (!e.repeat) commands.cut();
      } else if (isControlKey && e.key.toLowerCase() === "v") {
        e.preventDefault();
        if (!e.repeat && EDITOR_MUTATION_CAPABILITIES.pasteNodes) {
          commands.paste(getActiveCanvasLocalPoint(lastMousePosRef.current.x, lastMousePosRef.current.y));
        }
      } else if (isControlKey && e.key.toLowerCase() === "d") {
        e.preventDefault();
        if (!e.repeat && EDITOR_MUTATION_CAPABILITIES.duplicateNodes) commands.duplicateSelected();
      } else if (isControlKey && e.key.toLowerCase() === "s") {
        e.preventDefault();
        if (e.shiftKey) commands.saveGraphAs();
        else commands.saveGraph();
      } else if (isControlKey && e.key.toLowerCase() === "o") {
        e.preventDefault();
        commands.importGraph();
      } else if (isControlKey && e.key.toLowerCase() === "n") {
        e.preventDefault();
        commands.addEvent(undefined, { openAfterCreate: true });
      } else if (isControlKey && e.key.toLowerCase() === "w") {
        e.preventDefault();
        const activeTabId = editorDockviewPort.getActivePanel()?.tab?.resourceRef;
        if (activeTabId) commands.closeTab(activeTabId);
      } else if (isControlKey && e.key === "Tab") {
        e.preventDefault();
        const gid = editorDockviewPort.getActiveGroupId();
        if (gid) {
          const tabs = listDockviewGroupTabs(gid);
          const activeTabId = getActiveLayoutTab(gid)?.activeTabId;
          if (tabs.length > 1 && activeTabId) {
            const currentIndex = tabs.findIndex((t) => t.id === activeTabId);
            const nextIndex = e.shiftKey
              ? (currentIndex - 1 + tabs.length) % tabs.length
              : (currentIndex + 1) % tabs.length;
            commands.setActiveTabId(tabs[nextIndex].id);
          }
        }
      } else if (isControlKey && e.key === "\\") {
        e.preventDefault();
        const gid = editorDockviewPort.getActiveGroupId();
        if (gid) commands.splitEditorRight(gid);
      } else if (isControlKey && e.key.toLowerCase() === 'b') {
        e.preventDefault();
        toggleSidebarVisibility();
      } else if (isControlKey && e.key.toLowerCase() === 'i') {
        e.preventDefault();
        toggleDetailVisibility();
      } else if (isControlKey && e.key === "`") {
        e.preventDefault();
        togglePanelCollapsed();
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
        toggleZenMode();
      }
    };

    const handleKeyUp = (e: KeyboardEvent) => {
      setModifierKeys({ altKey: e.altKey, ctrlKey: e.ctrlKey, shiftKey: e.shiftKey });
    };

    const handleBlur = () => {
      resetModifierKeys();
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
  }, [commands]);
}
