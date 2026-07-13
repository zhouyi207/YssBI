import type { RefObject } from 'react';
import { useGestureStore } from '@/features/core/gesture';
import { commitViewport, setViewportLive, editorViewportScope } from '@/features/core/viewport';
import { applyCanvasDetailFocus } from '@/features/core/editor/detail/detailFocusCommands';
import { executeCommand } from '@/features/core/history';
import type { Pin } from '@/shared/types/domain';
import type { EditorViewport } from '@/features/core/viewport';
import type { EditorGesture } from '@/shared/types/ui';
import { logger } from '@/utils/appLogger';
import { CONTEXT_MENU_MOVE_THRESHOLD_PX } from '@/app/appConfig/default';
import { addGlobalEventListener } from '@/shared/utils/globalEvent';
import { getCanvasWorldPoint, getGestureScreenMovement, resolveTabId } from './canvasInteractionUtils';
import {
  collectSelectionHitTargets,
  hitTestSelection,
  queryCanvasElement,
  syncSelectionPreview,
  clearAllSelectionPreview,
} from './selectionHitTargets';
import {
  getSelectionSession,
  updateSelectionSession,
  endSelectionSession,
  abortSelectionSession,
  getSelectionPreviewIds,
  setSelectionPreviewIds,
  selectionScreenRect,
  selectionSessionMoved,
} from './selectionSession';

export type CanvasPointerLoopDeps = {
  activeGroupIdRef: RefObject<string>;
  activeTabIdRef: RefObject<string | null>;
  viewportRef: RefObject<EditorViewport>;
  setSelectedNodeIds: (updater: string[] | ((prev: string[]) => string[]), targetGroupId?: string) => void;
  connectPins: (groupId: string, pinA: string, pinB: string) => Promise<void>;
  persistViewport: (scope?: { groupId: string; graphPath: string } | null) => void;
  setContextMenu: (menu: { x: number; y: number; visible: boolean }) => void;
  setPendingConnection: (pin: Pin | null) => void;
};

let attachCount = 0;
let removeWindowListeners: (() => void) | null = null;
const depsRef: { current: CanvasPointerLoopDeps | null } = { current: null };

function installPointerLoop(): () => void {
  let rAFId: number | null = null;
  let latestEvent: PointerEvent | null = null;

  const processSelectionFrame = (e: PointerEvent) => {
    const session = getSelectionSession();
    if (!session.active) return;

    updateSelectionSession(e.clientX, e.clientY);
    const canvasEl = queryCanvasElement(session.groupId);
    if (!canvasEl) return;

    const rect = selectionScreenRect({
      ...session,
      currentX: e.clientX,
      currentY: e.clientY,
    });
    const newSelectedIds = hitTestSelection(collectSelectionHitTargets(canvasEl), rect);
    syncSelectionPreview(canvasEl, getSelectionPreviewIds(), newSelectedIds);
    setSelectionPreviewIds(newSelectedIds);
  };

  const processGestureFrame = (e: PointerEvent) => {
    const deps = depsRef.current;
    if (!deps) return;

    const g = useGestureStore.getState().gesture;
    if (!g) return;

    let nextGesture: EditorGesture = null;

    if (g.type === 'pan') {
      const dx = e.clientX - g.lastX;
      const dy = e.clientY - g.lastY;
      const layoutGroupId = g.groupId || deps.activeGroupIdRef.current;
      const graphPath = resolveTabId(layoutGroupId, deps.activeTabIdRef);
      if (graphPath) {
        setViewportLive(editorViewportScope(layoutGroupId, graphPath), (prev) => ({
          ...prev,
          x: prev.x + dx,
          y: prev.y + dy,
        }));
      }
      nextGesture = { ...g, lastX: e.clientX, lastY: e.clientY, moved: true };
    } else if (g.type === 'connect') {
      const gid = g.groupId || deps.activeGroupIdRef.current;
      const tid = resolveTabId(gid, deps.activeTabIdRef);
      const { x: worldX, y: worldY } = getCanvasWorldPoint(gid, tid, e.clientX, e.clientY);
      nextGesture = { ...g, currentX: e.clientX, currentY: e.clientY, worldX, worldY };
      useGestureStore.getState().setGesture(nextGesture);
      nextGesture = null;
    } else if (g.type === 'drag') {
      const viewport = deps.viewportRef.current ?? { scale: 1 };
      const scale = viewport.scale || 1;
      const dx = (e.clientX - g.lastX) / scale;
      const dy = (e.clientY - g.lastY) / scale;

      let moved = g.moved;
      let lastX = g.lastX;
      let lastY = g.lastY;
      const prevDelta = g.dragDelta || { x: 0, y: 0 };
      let dragDelta = prevDelta;

      if (Math.abs(dx) > 0.01 || Math.abs(dy) > 0.01) {
        moved = true;
        dragDelta = { x: prevDelta.x + dx, y: prevDelta.y + dy };
        lastX = e.clientX;
        lastY = e.clientY;
      }
      nextGesture = { ...g, moved, lastX, lastY, dragDelta };
    }

    if (nextGesture) {
      useGestureStore.getState().setGesture(nextGesture);
    }
  };

  const flushMove = () => {
    if (!latestEvent) return;
    const e = latestEvent;
    latestEvent = null;
    rAFId = null;

    if (getSelectionSession().active) {
      processSelectionFrame(e);
    } else {
      processGestureFrame(e);
    }
  };

  const onMove = (e: PointerEvent) => {
    latestEvent = e;
    if (rAFId === null) {
      rAFId = requestAnimationFrame(flushMove);
    }
  };

  const finalizeSelection = (e: PointerEvent) => {
    const deps = depsRef.current;
    const session = getSelectionSession();
    if (!session.active || !deps) return false;

    const gid = session.groupId;
    const canvasEl = queryCanvasElement(gid);
    const finalSession = { ...session, currentX: e.clientX, currentY: e.clientY };
    const hadMovement = selectionSessionMoved(finalSession, CONTEXT_MENU_MOVE_THRESHOLD_PX);
    const rect = selectionScreenRect(finalSession);
    const newSelectedIds = canvasEl
      ? hitTestSelection(collectSelectionHitTargets(canvasEl), rect)
      : [];

    if (canvasEl) clearAllSelectionPreview(canvasEl);
    endSelectionSession();

    if (hadMovement) {
      deps.setSelectedNodeIds(newSelectedIds, gid);
      applyCanvasDetailFocus({ type: 'box-select', groupId: gid, selectedIds: newSelectedIds });
    } else {
      if (!session.preserveSelection) deps.setSelectedNodeIds([], gid);
      applyCanvasDetailFocus({ type: 'blank-click', groupId: gid });
    }

    return true;
  };

  const onUp = (e: PointerEvent) => {
    const deps = depsRef.current;
    if (!deps) return;

    if (rAFId) {
      cancelAnimationFrame(rAFId);
      rAFId = null;
    }

    finalizeSelection(e);

    const g = useGestureStore.getState().gesture;
    if (!g) return;

    if (g.type === 'pan') {
      if (!g.moved && e.button === 2) {
        deps.setContextMenu({ x: e.clientX, y: e.clientY, visible: true });
      } else if (g.moved) {
        const layoutGroupId = g.groupId || deps.activeGroupIdRef.current;
        const graphPath = resolveTabId(layoutGroupId, deps.activeTabIdRef);
        if (graphPath) {
          commitViewport(editorViewportScope(layoutGroupId, graphPath));
          deps.persistViewport({ groupId: layoutGroupId, graphPath });
        }
      }
    } else if (g.type === 'connect') {
      const gid = g.groupId || deps.activeGroupIdRef.current;
      const target = (e.target as HTMLElement).closest('[data-pin-id]');
      if (target) deps.connectPins(gid, g.startPin.id, target.getAttribute('data-pin-id')!);
      else {
        deps.setPendingConnection(g.startPin);
        deps.setContextMenu({ x: e.clientX, y: e.clientY, visible: true });
      }
    } else if (g.type === 'drag') {
      const gid = g.groupId || deps.activeGroupIdRef.current;
      if (g.moved) {
        const delta = g.dragDelta || { x: 0, y: 0 };
        if (Math.abs(delta.x) > 0.001 || Math.abs(delta.y) > 0.001) {
          const dragIds = g.dragNodeIds || [];
          const tid = resolveTabId(gid, deps.activeTabIdRef);
          if (tid && dragIds.length > 0) {
            executeCommand(
              tid,
              'MoveNodes',
              { nodeIds: dragIds, delta },
              { mergeKey: `move-${[...dragIds].sort().join(',')}` },
            ).catch((err) =>
              logger.graph.warn(
                `MoveNodes command failed: ${err instanceof Error ? err.message : String(err)}`,
                'CanvasInteraction',
              ),
            );
          }
        }
      } else {
        applyCanvasDetailFocus({ type: 'node-click', groupId: gid, nodeId: g.nodeId! });
      }
    }

    useGestureStore.getState().endConnection();
    const hadMovement = getGestureScreenMovement(g, deps.viewportRef.current?.scale ?? 1);
    useGestureStore.getState().clearGesture(hadMovement);
  };

  const cleanupPointerMove = addGlobalEventListener(window, 'pointermove', onMove);
  const cleanupPointerUp = addGlobalEventListener(window, 'pointerup', onUp);

  return () => {
    cleanupPointerMove();
    cleanupPointerUp();
    if (rAFId) cancelAnimationFrame(rAFId);
    const session = getSelectionSession();
    if (session.active) abortSelectionSession(session.groupId);
  };
}

/** One window-level pointer loop shared by all canvases (ref-counted). */
export function attachCanvasPointerLoop(deps: CanvasPointerLoopDeps): () => void {
  depsRef.current = deps;
  attachCount += 1;
  if (attachCount === 1) {
    removeWindowListeners = installPointerLoop();
  }
  return () => {
    attachCount -= 1;
    if (attachCount === 0 && removeWindowListeners) {
      removeWindowListeners();
      removeWindowListeners = null;
      depsRef.current = null;
    }
  };
}
