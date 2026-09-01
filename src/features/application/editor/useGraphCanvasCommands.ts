import { useMemo } from "react";
import type { EditorSessionCanvasActions } from "./editorSessionTypes";
import { revealInspect } from "./rightSidebarActions";
import { isEditorCommandTargetCurrent, type EditorCommandTarget } from "./editorCommandFocus";
import { collectCanvasNodeWorldBounds } from "@/features/core/canvas";
import { useGraphDataStore } from "@/features/core/dataStore";
import {
  getEditorGroupGraphSelection,
  updateEditorGroupSelectedNodeIds,
} from "@/features/core/layout/layoutTabQueries";
import { getDocumentState } from "@/features/core/resource";
import {
  commitViewport,
  editorViewportScope,
  fitWorldBounds,
  getViewport,
  persistGraphViewport,
  setViewportLive,
  type ViewportScope,
} from "@/features/core/viewport";

interface ActiveGraphContext {
  target: EditorCommandTarget;
  graphPath: string;
  groupId: string;
  scope: ViewportScope;
}

interface ActiveGraphCanvas extends ActiveGraphContext {
  canvasElement: HTMLElement;
}

function activeGraphContext(target: EditorCommandTarget): ActiveGraphContext | null {
  if (!isEditorCommandTargetCurrent(target)) return null;
  if (target.resourceKind !== "event" && target.resourceKind !== "function") return null;
  const graphPath = target.resourceRef;
  if (!getDocumentState({ id: graphPath, kind: target.resourceKind })?.loaded) return null;
  if (!useGraphDataStore.getState().graphEntities[graphPath]) return null;

  return {
    target,
    graphPath,
    groupId: target.groupId,
    scope: editorViewportScope(target.groupId, graphPath),
  };
}

function activeGraphCanvas(target: EditorCommandTarget): ActiveGraphCanvas | null {
  const context = activeGraphContext(target);
  if (!context) return null;
  const canvasElement = document.querySelector<HTMLElement>(
    `[data-editor-panel-instance-id="${context.target.panelInstanceId}"]`,
  );
  return canvasElement ? { ...context, canvasElement } : null;
}

function fitCanvasNodes(context: ActiveGraphCanvas, nodeIds?: readonly string[]): boolean {
  const viewport = getViewport(context.scope);
  const bounds = collectCanvasNodeWorldBounds({
    canvasElement: context.canvasElement,
    viewport,
    nodeIds,
  });
  if (!bounds || !isEditorCommandTargetCurrent(context.target)) return false;

  const canvasRect = context.canvasElement.getBoundingClientRect();
  const next = fitWorldBounds(bounds, { width: canvasRect.width, height: canvasRect.height });
  setViewportLive(context.scope, next);
  commitViewport(context.scope);
  persistGraphViewport(context.scope);
  return true;
}

export function useGraphCanvasCommands(): EditorSessionCanvasActions {
  return useMemo(
    () => ({
      async selectAllNodes(target: EditorCommandTarget): Promise<boolean> {
        const context = activeGraphContext(target);
        if (!context) return false;
        const bucket = useGraphDataStore.getState().graphEntities[context.graphPath];
        const selectableNodeIds = bucket.graphNodes.filter(
          (nodeId) => bucket.nodes[nodeId]?.capabilities?.managed === false,
        );
        if (selectableNodeIds.length === 0 || !isEditorCommandTargetCurrent(target)) return false;
        const update = updateEditorGroupSelectedNodeIds(selectableNodeIds, context.groupId);
        if (!update || !isEditorCommandTargetCurrent(target)) return false;
        await revealInspect(context.graphPath, update.nodeIds);
        return true;
      },

      focusSelectedNodes(target: EditorCommandTarget): boolean {
        const context = activeGraphCanvas(target);
        if (!context) return false;
        const nodeIds = [...getEditorGroupGraphSelection(context.groupId).nodeIds];
        if (nodeIds.length === 0) return false;
        return fitCanvasNodes(context, nodeIds);
      },

      fitCompleteGraph(target: EditorCommandTarget): boolean {
        const context = activeGraphCanvas(target);
        return context ? fitCanvasNodes(context) : false;
      },
    }),
    [],
  );
}
