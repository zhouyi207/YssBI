import { useEffect, useRef } from "react";
import { ui } from "@/features/core/ui/ui";
import {
  clearEditorGroupGraphSelection,
  getEditorGroupGraphSelection,
} from "@/modules/workbench/public";
import { getViewport, editorViewportScope } from "@/features/core/viewport";
import { useModifierKeyStore } from "@/features/core/keyboard";
import { useWorkbenchUiStore } from "@/modules/workbench/public";
import { addGlobalEventListener } from "@/shared/utils/globalEvent";
import { isGraphSaving, useGraphEditingStore } from "@/features/core/graphEditing";
import {
  getCanvasInteraction,
  useGraphInteractionStore,
} from "@/features/core/graphInteraction/graphInteractionStore";
import { cancelCanvasInteraction } from "@/features/core/canvas/canvasInteractionCleanup";
import { useEditorStore } from "@/features/core/editor";
import type { WorkbenchCommandCapability } from "./workbenchCommandCapability";
import {
  captureActiveEditorCommandTarget,
  captureEditorShortcutTarget,
  keyboardPanelInstanceId,
  isEditorCommandTargetCurrent,
  shouldIgnoreEditorShortcutEvent,
  type EditorCommandTarget,
} from "./editorCommandFocus";
import { workbenchLayoutControl } from "@/modules/workbench/public";
import { workbenchLayoutRead } from "@/modules/workbench/public";
import { requestCloseWorkbenchPanel } from "./workbenchPanelClose";
import {
  toggleActivityWorkbenchGroup,
  toggleBottomWorkbenchGroup,
} from "@/modules/workbench/public";

function currentEditorCommandTarget(event: KeyboardEvent): EditorCommandTarget | null {
  const target = captureEditorShortcutTarget(event);
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

function cyclePhysicalPanel(event: KeyboardEvent, backward: boolean): boolean {
  const id = keyboardPanelInstanceId(event);
  const activePanel = id ? workbenchLayoutRead.getPanel(id) : workbenchLayoutRead.getActivePanel();
  if (!activePanel) return false;
  const panels = workbenchLayoutRead.listGroupPanels(activePanel.groupId);
  if (panels.length < 2) return false;
  const currentIndex = panels.findIndex(
    (panel) => panel.panelInstanceId === activePanel.panelInstanceId,
  );
  if (currentIndex < 0) return false;
  const offset = backward ? -1 : 1;
  const nextIndex = (currentIndex + offset + panels.length) % panels.length;
  const nextId = panels[nextIndex].panelInstanceId;
  void workbenchLayoutControl.activate(nextId).then((activated) => {
    if (!activated || !workbenchLayoutRead.getPanel(nextId)?.visible) return;
    document
      .querySelector<HTMLElement>(`[data-panel-instance-id="${CSS.escape(nextId)}"]`)
      ?.focus({ preventScroll: true });
  });
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
        const target = currentEditorCommandTarget(event);
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

      if (isControlKey && key === ",") {
        event.preventDefault();
        ui.showSettings();
        return;
      }

      if (event.key === "F1") {
        event.preventDefault();
        useWorkbenchUiStore.getState().setNodeDocumentationOpen(true);
        return;
      }

      if (!event.repeat && isControlKey && key === "a") {
        const target = currentEditorCommandTarget(event);
        if (!target) return;
        event.preventDefault();
        void commands.selectAllNodes(target);
        return;
      }

      if (!event.repeat && !isControlKey && !event.altKey && !event.shiftKey && key === "f") {
        const target = currentEditorCommandTarget(event);
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
        const target = currentEditorCommandTarget(event);
        if (target && commands.fitCompleteGraph(target)) event.preventDefault();
        return;
      }

      if (event.key === "Delete" || event.key === "Backspace") {
        const target = currentEditorCommandTarget(event);
        if (!target || isGraphSaving(target.resourceRef)) return;
        event.preventDefault();
        void commands.deleteSelected(target);
        return;
      }

      if (isControlKey && key === "z") {
        const target = currentEditorCommandTarget(event);
        if (!target) return;
        const session = useGraphEditingStore.getState().sessions[target.resourceRef];
        const canUndo = Boolean(session?.canUndo);
        const canRedo = Boolean(session?.canRedo);
        if (!session?.saving && (event.shiftKey ? canRedo : canUndo)) {
          event.preventDefault();
          if (event.shiftKey) void commands.redo(target);
          else void commands.undo(target);
        }
        return;
      }

      if (isControlKey && key === "y") {
        const target = currentEditorCommandTarget(event);
        if (!target) return;
        const session = useGraphEditingStore.getState().sessions[target.resourceRef];
        if (session?.canRedo && !session.saving) {
          event.preventDefault();
          void commands.redo(target);
        }
        return;
      }

      if (isControlKey && key === "c") {
        const target = currentEditorCommandTarget(event);
        if (!target || isGraphSaving(target.resourceRef)) return;
        event.preventDefault();
        if (!event.repeat) void commands.copy(target);
        return;
      }

      if (isControlKey && key === "x") {
        const target = currentEditorCommandTarget(event);
        if (!target || isGraphSaving(target.resourceRef)) return;
        event.preventDefault();
        if (!event.repeat) void commands.cut(target);
        return;
      }

      if (isControlKey && key === "v") {
        const target = currentEditorCommandTarget(event);
        if (!target || isGraphSaving(target.resourceRef)) return;
        event.preventDefault();
        if (!event.repeat) {
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
        const target = currentEditorCommandTarget(event);
        if (!target || isGraphSaving(target.resourceRef)) return;
        event.preventDefault();
        if (!event.repeat) {
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
        const target = captureActiveEditorCommandTarget();
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
        const id = keyboardPanelInstanceId(event);
        const activePanel = id
          ? workbenchLayoutRead.getPanel(id)
          : workbenchLayoutRead.getActivePanel();
        if (!activePanel) return;
        event.preventDefault();
        void requestCloseWorkbenchPanel(activePanel.panelInstanceId);
        return;
      }

      if (isControlKey && event.key === "Tab") {
        if (cyclePhysicalPanel(event, event.shiftKey)) event.preventDefault();
        return;
      }

      if (isControlKey && event.key === "\\") {
        const target = captureActiveEditorCommandTarget();
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
