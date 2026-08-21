import { useMemo } from 'react';
import type { EditorSessionCanvasActions } from './editorSessionTypes';
import { focusCanvasSelection } from './rightSidebarActions';
import { collectCanvasNodeWorldBounds } from '@/features/core/canvas';
import { useGraphDataStore } from '@/features/core/dataStore';
import { editorDockviewPort } from '@/features/core/dockview';
import {
  getActiveLayoutTab,
  getEditorGroupGraphSelection,
  updateEditorGroupSelectedNodeIds,
} from '@/features/core/layout/layoutTabQueries';
import { getDocumentState } from '@/features/core/resource';
import {
  commitViewport,
  editorViewportScope,
  fitWorldBounds,
  getViewport,
  persistGraphViewport,
  setViewportLive,
  type ViewportScope,
} from '@/features/core/viewport';

interface ActiveGraphContext {
  graphPath: string;
  groupId: string;
  scope: ViewportScope;
}

interface ActiveGraphCanvas extends ActiveGraphContext {
  canvasElement: HTMLElement;
}

function activeGraphContext(): ActiveGraphContext | null {
  const groupId = editorDockviewPort.getActiveGroupId();
  if (!groupId) return null;

  const active = getActiveLayoutTab(groupId);
  if (!active || (active.tab.type !== 'event' && active.tab.type !== 'function')) return null;
  const graphPath = active.activeTabId;
  if (!getDocumentState({ id: graphPath, kind: active.tab.type })?.loaded) return null;
  if (!useGraphDataStore.getState().graphEntities[graphPath]) return null;

  return { graphPath, groupId, scope: editorViewportScope(groupId, graphPath) };
}

function activeGraphCanvas(): ActiveGraphCanvas | null {
  const context = activeGraphContext();
  if (!context) return null;
  const canvasElement = [...document.querySelectorAll<HTMLElement>('[data-editor-group-id]')]
    .find((element) => element.dataset.editorGroupId === context.groupId);
  return canvasElement ? { ...context, canvasElement } : null;
}

function fitCanvasNodes(context: ActiveGraphCanvas, nodeIds?: readonly string[]): boolean {
  const viewport = getViewport(context.scope);
  const bounds = collectCanvasNodeWorldBounds({
    canvasElement: context.canvasElement,
    viewport,
    nodeIds,
  });
  if (!bounds) return false;

  const canvasRect = context.canvasElement.getBoundingClientRect();
  const next = fitWorldBounds(bounds, { width: canvasRect.width, height: canvasRect.height });
  setViewportLive(context.scope, next);
  commitViewport(context.scope);
  persistGraphViewport(context.scope);
  return true;
}

export function useGraphCanvasCommands(): EditorSessionCanvasActions {
  return useMemo(() => ({
    selectAllNodes(): boolean {
      const context = activeGraphContext();
      if (!context) return false;
      const bucket = useGraphDataStore.getState().graphEntities[context.graphPath];
      const selectableNodeIds = bucket.graphNodes.filter(
        (nodeId) => bucket.nodes[nodeId]?.capabilities?.managed === false,
      );
      if (selectableNodeIds.length === 0) return false;
      const update = updateEditorGroupSelectedNodeIds(selectableNodeIds, context.groupId);
      if (!update) return false;
      const active = getActiveLayoutTab(update.groupId);
      if (active?.tab.type === 'event' || active?.tab.type === 'function') {
        focusCanvasSelection(active.activeTabId, update.nodeIds);
      }
      return true;
    },

    focusSelectedNodes(): boolean {
      const context = activeGraphCanvas();
      if (!context) return false;
      const nodeIds = [...getEditorGroupGraphSelection(context.groupId).nodeIds];
      if (nodeIds.length === 0) return false;
      return fitCanvasNodes(context, nodeIds);
    },

    fitCompleteGraph(): boolean {
      const context = activeGraphCanvas();
      return context ? fitCanvasNodes(context) : false;
    },
  }), []);
}
