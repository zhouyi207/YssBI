import { useEffect, useRef } from "react";
import {
  clearEditorGroupGraphSelection,
  getEditorGroupGraphSelection,
} from "@/features/core/editor/editorGroupSelection";
import { getViewport, editorViewportScope } from "@/features/core/viewport";
import { useModifierKeyStore } from "@/features/core/keyboard";
import { useWorkbenchUiStore } from "@/modules/workbench/public";
import { addGlobalEventListener } from "@/shared/utils/globalEvent";
import { useHistoryStore } from "@/features/core/history";
import {
  getCanvasInteraction,
  useGraphInteractionStore,
} from "@/features/core/graphInteraction/graphInteractionStore";
import { cancelCanvasInteraction } from "@/features/core/canvas/canvasInteractionCleanup";
import { useEditorStore } from "@/features/core/editor";
import { EDITOR_MUTATION_CAPABILITIES } from "./editorMutationAvailability";
import type { WorkbenchCommandCapability } from "./workbenchCommandCapability";
import {
  captureActiveEditorCommandTarget,
  isEditorCommandTargetCurrent,
  shouldIgnoreEditorShortcutEvent,
  type EditorCommandTarget,
} from "./editorCommandFocus";
import { workbenchDockviewControl } from "@/features/core/dockview/workbenchControl";
import { workbenchDockviewRead } from "@/features/core/dockview/workbenchRead";
import { requestCloseWorkbenchPanel } from "./workbenchPanelClose";
import {
  toggleActivityWorkbenchGroup,
  toggleBottomWorkbenchGroup,
  toggleWorkbenchView,
} from "@/features/application/layout/workbenchLayoutActions";

function currentEditorCommandTarget(): EditorCommandTarget | null {
  const target = captureActiveEditorCommandTarget();
  return target && isEditorCommandTargetCurrent(target) ? target : null;
}

function getActiveCanvasLocalPoint(target: EditorCommandTarget, clientX: number, clientY: number) {
  const element = document.querySelector(
    `[data-editor-panel-instance-id="${target.panelInstanceId}"]`,
  );
  if (!(element instanceof HTMLElement)) return { x: 0, y: 0 };
  const rect = element.getBoundingClientRect();
  const viewport = getViewport(editorViewportScope(target.groupId, target.resourceRef));
  return {
    x: (clientX - rect.left - viewport.x) / viewport.scale,
    y: (clientY - rect.top - viewport.y) / viewport.scale,
  };
}

function cyclePhysicalPanel(backward: boolean): boolean {
  const activePanel = workbenchDockviewRead.getActivePanel();
  if (!activePanel) return false;
  const panels = workbenchDockviewRead.listGroupPanels(activePanel.groupId);
  if (panels.length < 2) return false;
  const currentIndex = panels.findIndex(
    (panel) => panel.panelInstanceId === activePanel.panelInstanceId,
  );
  if (currentIndex < 0) return false;
  const offset = backward ? -1 : 1;
  const nextIndex = (currentIndex + offset + panels.length) % panels.length;
  void workbenchDockviewControl.activate(panels[nextIndex].panelInstanceId);
  return true;
}

/** Mounts the workbench window's single ordered global keyboard shortcut listener set. */
export function useEditorKeyboard(commands: WorkbenchCommandCapability): void {
  const lastMousePosRef = useRef({ x: 0, y: 0 });

  useEffect(() => {
    const setModifierKeys = useModifierKeyStore.getState().setModifierKeys;
    const resetModifierKeys = useModifierKeyStore.getState().resetModifierKeys;

    const handlePointerMove = (event: PointerEvent) => {
      lastMousePosRef.current = { x: event.clientX, y: event.clientY };
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      setModifierKeys({
        altKey: event.altKey,
        ctrlKey: event.ctrlKey,
        shiftKey: event.shiftKey,
      });

      if (shouldIgnoreEditorShortcutEvent(event)) return;

      const isControlKey = event.ctrlKey || event.metaKey;
      const key = event.key.toLowerCase();

      if (event.key === "Escape") {
        const target = currentEditorCommandTarget();
        if (!target || target.resourceKind === "chart") return;
        const interaction = getCanvasInteraction(
          useGraphInteractionStore.getState(),
          target.resourceRef,
          target.groupId,
        );
        if (interaction.type !== "idle") {
          event.preventDefault();
          cancelCanvasInteraction(target.resourceRef, target.groupId);
          if (interaction.type === "pendingNodeCreation") {
            useEditorStore.getState().setContextMenu(null);
          }
          return;
        }
        const selection = getEditorGroupGraphSelection(target.groupId);
        if (selection.connectionIds.size > 0 || selection.nodeIds.size > 0) {
          event.preventDefault();
          clearEditorGroupGraphSelection(target.groupId);
        }
        return;
      }

      if (event.key === "F1") {
        event.preventDefault();
        useWorkbenchUiStore.getState().setNodeDocumentationOpen(true);
        return;
      }

      if (!event.repeat && isControlKey && key === "a") {
        const target = currentEditorCommandTarget();
        if (!target) return;
        event.preventDefault();
        void commands.selectAllNodes(target);
        return;
      }

      if (!event.repeat && !isControlKey && !event.altKey && !event.shiftKey && key === "f") {
        const target = currentEditorCommandTarget();
        if (target && commands.focusSelectedNodes(target)) event.preventDefault();
        return;
      }

      if (
        !event.repeat &&
        !isControlKey &&
        !event.altKey &&
        !event.shiftKey &&
        event.key === "Home"
      ) {
        const target = currentEditorCommandTarget();
        if (target && commands.fitCompleteGraph(target)) event.preventDefault();
        return;
      }

      if (event.key === "Delete" || event.key === "Backspace") {
        const target = currentEditorCommandTarget();
        if (!target) return;
        event.preventDefault();
        void commands.deleteSelected(target);
        return;
      }

      if (isControlKey && key === "z") {
        const target = currentEditorCommandTarget();
        if (!target) return;
        const { canUndo, canRedo, pending } = useHistoryStore.getState();
        if (event.shiftKey ? canRedo && !pending : canUndo && !pending) {
          event.preventDefault();
          if (event.shiftKey) void commands.redo(target);
          else void commands.undo(target);
        }
        return;
      }

      if (isControlKey && key === "y") {
        const target = currentEditorCommandTarget();
        if (!target) return;
        const { canRedo, pending } = useHistoryStore.getState();
        if (canRedo && !pending) {
          event.preventDefault();
          void commands.redo(target);
        }
        return;
      }

      if (isControlKey && key === "c") {
        const target = currentEditorCommandTarget();
        if (!target) return;
        event.preventDefault();
        if (!event.repeat) void commands.copy(target);
        return;
      }

      if (isControlKey && key === "x") {
        const target = currentEditorCommandTarget();
        if (!target) return;
        event.preventDefault();
        if (!event.repeat) void commands.cut(target);
        return;
      }

      if (isControlKey && key === "v") {
        const target = currentEditorCommandTarget();
        if (!target) return;
        event.preventDefault();
        if (!event.repeat && EDITOR_MUTATION_CAPABILITIES.pasteNodes) {
          const point = getActiveCanvasLocalPoint(
            target,
            lastMousePosRef.current.x,
            lastMousePosRef.current.y,
          );
          void commands.paste(point, target);
        }
        return;
      }

      if (isControlKey && key === "d") {
        const target = currentEditorCommandTarget();
        if (!target) return;
        event.preventDefault();
        if (!event.repeat && EDITOR_MUTATION_CAPABILITIES.duplicateNodes) {
          void commands.duplicateSelected(target);
        }
        return;
      }

      if (isControlKey && key === "s") {
        if (event.shiftKey) {
          event.preventDefault();
          void commands.saveGraphAs();
          return;
        }
        const target = currentEditorCommandTarget();
        if (!target || !isEditorCommandTargetCurrent(target)) return;
        event.preventDefault();
        void commands.saveGraph(target);
        return;
      }

      if (isControlKey && key === "o") {
        event.preventDefault();
        void commands.importGraph();
        return;
      }

      if (isControlKey && key === "n") {
        event.preventDefault();
        void commands.addEvent(undefined, { openAfterCreate: true });
        return;
      }

      if (isControlKey && key === "w") {
        const activePanel = workbenchDockviewRead.getActivePanel();
        if (!activePanel) return;
        event.preventDefault();
        void requestCloseWorkbenchPanel(activePanel.panelInstanceId);
        return;
      }

      if (isControlKey && event.key === "Tab") {
        if (cyclePhysicalPanel(event.shiftKey)) event.preventDefault();
        return;
      }

      if (isControlKey && event.key === "\\") {
        const target = currentEditorCommandTarget();
        if (!target || !isEditorCommandTargetCurrent(target)) return;
        event.preventDefault();
        commands.splitEditorRight(target.groupId);
        return;
      }

      if (isControlKey && key === "b") {
        event.preventDefault();
        void toggleActivityWorkbenchGroup();
        return;
      }

      if (isControlKey && key === "i") {
        event.preventDefault();
        void toggleWorkbenchView("inspect");
        return;
      }

      if (isControlKey && event.key === "`") {
        event.preventDefault();
        void toggleBottomWorkbenchGroup();
      }
    };

    const handleKeyUp = (event: KeyboardEvent) => {
      setModifierKeys({
        altKey: event.altKey,
        ctrlKey: event.ctrlKey,
        shiftKey: event.shiftKey,
      });
    };

    const handleBlur = () => {
      resetModifierKeys();
    };

    const cleanupKeyDown = addGlobalEventListener(window, "keydown", handleKeyDown, {
      capture: true,
    });
    const cleanupKeyUp = addGlobalEventListener(window, "keyup", handleKeyUp, { capture: true });
    const cleanupPointerMove = addGlobalEventListener(window, "pointermove", handlePointerMove, {
      capture: true,
    });
    const cleanupBlur = addGlobalEventListener(window, "blur", handleBlur);

    return () => {
      cleanupKeyDown();
      cleanupKeyUp();
      cleanupPointerMove();
      cleanupBlur();
    };
  }, [commands]);
}
