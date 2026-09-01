import type { RefObject } from "react";
import { useGestureStore } from "@/features/core/gesture";
import { useGraphDataStore } from "@/features/core/dataStore/graphDataStore";
import {
  getCanvasInteraction,
  useGraphInteractionStore,
  type CanvasInteraction,
  type CanvasInteractionScope,
} from "@/features/core/graphInteraction/graphInteractionStore";
import { commitViewport, setViewportLive, editorViewportScope } from "@/features/core/viewport";
import { executeCommand } from "@/features/core/history";
import type { EditorViewport } from "@/features/core/viewport";
import { logger } from "@/features/core/observability/logger";
import type { CanvasInteractionHandlers } from "./canvasMutationContracts";
import { CONTEXT_MENU_MOVE_THRESHOLD_PX } from "@/shared/config-default";
import { addGlobalEventListener } from "@/shared/utils/globalEvent";
import { getCanvasWorldPoint, resolveTabId } from "./canvasInteractionUtils";
import { unionSelectionIds } from "./selectionSession";
import { resolveConnectionTarget, type ConnectionCandidate } from "./connectionInteraction";
import { measurePinConnectionAnchor } from "./pinConnectionAnchor";
import {
  cancelCanvasInteraction,
  registerCanvasInteractionCleanup,
  startCanvasInteraction,
} from "./canvasInteractionCleanup";
import { clearCanvasPointerScope, getCanvasPointerScope } from "./pointerScope";
import {
  collectSelectionHitTargets,
  hitTestSelection,
  queryCanvasElement,
  syncSelectionPreview,
  clearAllSelectionPreview,
} from "./selectionHitTargets";

export type CanvasPointerLoopDeps = {
  activeGroupIdRef: RefObject<string>;
  activeResourceRefRef: RefObject<string | null>;
  panelInstanceId: string;
  viewportRef: RefObject<EditorViewport>;
  setSelectedNodeIds: (
    updater: string[] | ((prev: string[]) => string[]),
    targetGroupId?: string,
  ) => void;
  persistViewport: (scope?: { groupId: string; graphPath: string } | null) => void;
  setContextMenu: (menu: { x: number; y: number; visible: boolean }) => void;
} & Pick<CanvasInteractionHandlers, "submitConnection" | "reportMutationFailure">;

let attachCount = 0;
let removeWindowListeners: (() => void) | null = null;
const depsRef: { current: CanvasPointerLoopDeps | null } = { current: null };
export { registerCanvasPointerScope } from "./pointerScope";

function activeInteraction(
  event: PointerEvent,
): [CanvasInteractionScope, CanvasInteraction] | null {
  const scope = getCanvasPointerScope();
  if (!scope || scope.pointerId !== event.pointerId) return null;
  const interaction = getCanvasInteraction(
    useGraphInteractionStore.getState(),
    scope.graphPath,
    scope.groupId,
  );
  if (
    interaction.type === "idle" ||
    !("pointerId" in interaction.session) ||
    interaction.session.pointerId !== scope.pointerId
  )
    return null;
  return [scope, interaction];
}

function selectionRect(session: Extract<CanvasInteraction, { type: "selecting" }>["session"]) {
  return {
    x1: Math.min(session.startX, session.currentX),
    y1: Math.min(session.startY, session.currentY),
    x2: Math.max(session.startX, session.currentX),
    y2: Math.max(session.startY, session.currentY),
  };
}

function installPointerLoop(): () => void {
  let rAFId: number | null = null;
  let latestEvent: PointerEvent | null = null;
  const selectionPreviewIds = new Map<string, string[]>();
  const selectionCleanupRegistrations = new Map<string, () => void>();
  const scopeKey = (scope: CanvasInteractionScope) =>
    `${scope.graphPath}\u0000${scope.groupId}\u0000${scope.panelInstanceId}`;
  const ensureSelectionCleanup = (scope: CanvasInteractionScope) => {
    const key = scopeKey(scope);
    if (selectionCleanupRegistrations.has(key)) return;
    const unregister = registerCanvasInteractionCleanup(
      { ...scope, interactionType: "selecting" },
      () => {
        const canvas = queryCanvasElement(scope.panelInstanceId);
        if (canvas) clearAllSelectionPreview(canvas);
        selectionPreviewIds.delete(key);
        selectionCleanupRegistrations.delete(key);
      },
    );
    selectionCleanupRegistrations.set(key, unregister);
  };

  const selectionScopeIsValid = (
    deps: CanvasPointerLoopDeps,
    scope: CanvasInteractionScope,
    session: Extract<CanvasInteraction, { type: "selecting" }>["session"],
  ) => {
    const currentScope = getCanvasPointerScope();
    return (
      currentScope?.graphPath === scope.graphPath &&
      currentScope.groupId === scope.groupId &&
      currentScope.panelInstanceId === scope.panelInstanceId &&
      currentScope.pointerId === scope.pointerId &&
      session.groupId === scope.groupId &&
      session.pointerId === scope.pointerId &&
      deps.panelInstanceId === scope.panelInstanceId &&
      resolveTabId(deps.activeResourceRefRef) === scope.graphPath &&
      queryCanvasElement(scope.panelInstanceId) !== null
    );
  };

  const processConnectionFrame = (
    scope: CanvasInteractionScope,
    interaction: Extract<CanvasInteraction, { type: "drawingConnection" | "movingConnections" }>,
    event: PointerEvent,
  ) => {
    const { graphPath } = scope;
    const { session } = interaction;
    const canvas = queryCanvasElement(scope.panelInstanceId);
    if (!canvas) return;
    const bucket = useGraphDataStore.getState().graphEntities[graphPath];
    if (!bucket) {
      cancelCanvasInteraction(graphPath, session.groupId);
      clearCanvasPointerScope(graphPath);
      return;
    }
    const candidates: ConnectionCandidate[] = [];
    canvas.querySelectorAll<HTMLElement>("[data-pin-id]").forEach((element) => {
      const anchor = measurePinConnectionAnchor(element);
      if (!anchor) return;
      const pin = bucket.pins[anchor.pinId];
      if (!pin) return;
      candidates.push({
        pin,
        center: anchor.center,
        connectionIds: bucket.pinConnections[pin.id] ?? [],
      });
    });
    const target = resolveConnectionTarget({
      source: session.source,
      sourceConnectionIds: bucket.pinConnections[session.source.id] ?? [],
      pointer: { x: event.clientX, y: event.clientY },
      candidates,
    });
    const pointerWorld = getCanvasWorldPoint(
      session.groupId,
      graphPath,
      event.clientX,
      event.clientY,
      scope.panelInstanceId,
    );
    const snappedWorld = target.snappedCenter
      ? getCanvasWorldPoint(
          session.groupId,
          graphPath,
          target.snappedCenter.x,
          target.snappedCenter.y,
          scope.panelInstanceId,
        )
      : null;
    useGraphInteractionStore.getState().updateInteraction(graphPath, session.groupId, (current) =>
      current.type === interaction.type
        ? {
            ...current,
            session: {
              ...current.session,
              screenX: event.clientX,
              screenY: event.clientY,
              worldX: pointerWorld.x,
              worldY: pointerWorld.y,
              hoveredTarget: target.hoveredTarget,
              snappedTarget: target.snappedTarget,
              snappedWorld,
              feedback: target.feedback,
            },
          }
        : current,
    );
  };

  const processFrame = (event: PointerEvent) => {
    const deps = depsRef.current;
    if (!deps) return;
    const active = activeInteraction(event);
    if (!active) return;
    const [scope, interaction] = active;
    if (scope.panelInstanceId !== deps.panelInstanceId) {
      cancelCanvasInteraction(scope.graphPath, scope.groupId);
      clearCanvasPointerScope(scope.graphPath);
      return;
    }
    const { graphPath } = scope;
    const store = useGraphInteractionStore.getState();

    if (interaction.type === "panning") {
      const session = interaction.session;
      const crossedDragThreshold =
        Math.abs(event.clientX - session.startX) > CONTEXT_MENU_MOVE_THRESHOLD_PX ||
        Math.abs(event.clientY - session.startY) > CONTEXT_MENU_MOVE_THRESHOLD_PX;
      if (!session.moved && !crossedDragThreshold) return;

      const dx = event.clientX - session.lastX;
      const dy = event.clientY - session.lastY;
      setViewportLive(editorViewportScope(session.groupId, graphPath), (viewport) => ({
        ...viewport,
        x: viewport.x + dx,
        y: viewport.y + dy,
      }));
      store.updateInteraction(graphPath, session.groupId, () => ({
        type: "panning",
        session: { ...session, lastX: event.clientX, lastY: event.clientY, moved: true },
      }));
    } else if (interaction.type === "selecting") {
      if (!selectionScopeIsValid(deps, scope, interaction.session)) {
        cancelCanvasInteraction(graphPath, interaction.session.groupId);
        return;
      }
      const session = { ...interaction.session, currentX: event.clientX, currentY: event.clientY };
      const key = scopeKey(scope);
      ensureSelectionCleanup(scope);
      const canvas = queryCanvasElement(scope.panelInstanceId);
      if (canvas) {
        const hitIds = hitTestSelection(collectSelectionHitTargets(canvas), selectionRect(session));
        const ids = unionSelectionIds(session.baseNodeIds, hitIds);
        syncSelectionPreview(canvas, selectionPreviewIds.get(key) ?? [], ids);
        selectionPreviewIds.set(key, ids);
      }
      store.updateInteraction(graphPath, session.groupId, () => ({ type: "selecting", session }));
    } else if (interaction.type === "draggingNodes") {
      const session = interaction.session;
      const scale = deps.viewportRef.current?.scale || 1;
      const dx = (event.clientX - session.lastX) / scale;
      const dy = (event.clientY - session.lastY) / scale;
      if (!session.moved && dx === 0 && dy === 0) return;
      const delta = { x: session.delta.x + dx, y: session.delta.y + dy };
      const positions: Record<string, { x: number; y: number }> = {};
      for (const nodeId of session.nodeIds) {
        const position = useGraphDataStore.getState().getGraphNode(graphPath, nodeId)?.position;
        if (position)
          positions[nodeId] = {
            x: position.x + delta.x,
            y: position.y + delta.y,
          };
      }
      store.updateNodeDragFrame(graphPath, session.groupId, positions, {
        ...session,
        moved: true,
        lastX: event.clientX,
        lastY: event.clientY,
        delta,
      });
    } else if (
      interaction.type === "drawingConnection" ||
      interaction.type === "movingConnections"
    ) {
      processConnectionFrame(scope, interaction, event);
    }
  };

  const flushMove = () => {
    const event = latestEvent;
    latestEvent = null;
    rAFId = null;
    if (event) processFrame(event);
  };

  const onMove = (event: PointerEvent) => {
    if (getCanvasPointerScope()?.pointerId !== event.pointerId) return;
    latestEvent = event;
    if (rAFId === null) rAFId = requestAnimationFrame(flushMove);
  };

  const onUp = (event: PointerEvent) => {
    const deps = depsRef.current;
    if (!deps) return;
    const active = activeInteraction(event);
    if (!active) return;
    if (rAFId !== null) {
      cancelAnimationFrame(rAFId);
      rAFId = null;
    }
    latestEvent = null;
    const [scope] = active;
    const { graphPath, groupId } = scope;
    if (scope.panelInstanceId !== deps.panelInstanceId) {
      cancelCanvasInteraction(graphPath, groupId);
      clearCanvasPointerScope(graphPath);
      return;
    }
    processFrame(event);
    const interaction = getCanvasInteraction(
      useGraphInteractionStore.getState(),
      graphPath,
      groupId,
    );
    const store = useGraphInteractionStore.getState();

    if (interaction.type === "drawingConnection" || interaction.type === "movingConnections") {
      const { source, snappedTarget, feedback } = interaction.session;
      if (snappedTarget && feedback && feedback.kind !== "invalid") {
        const moving = interaction.type === "movingConnections";
        const intentType = moving ? "moveConnections" : "connect";
        const outcome = deps.submitConnection({
          graphPath,
          intent: intentType,
          sourcePinId: source.id,
          targetPinId: snappedTarget.id,
        });
        void Promise.resolve(outcome)
          .then((result) => {
            if (result.status === "failed" && result.message) {
              deps.reportMutationFailure({
                graphPath,
                intent: intentType,
                message: result.message,
              });
            }
          })
          .catch(() => {
            logger.graph.warn(
              `Graph mutation command failed graphPath=${graphPath} intent=${intentType}`,
              "CanvasInteraction",
            );
          });
        cancelCanvasInteraction(graphPath, groupId);
      } else if (
        interaction.type === "drawingConnection" &&
        !interaction.session.hoveredTarget &&
        !interaction.session.feedback
      ) {
        startCanvasInteraction(graphPath, {
          type: "pendingNodeCreation",
          session: {
            groupId: interaction.session.groupId,
            graphPath,
            source,
            screenX: event.clientX,
            screenY: event.clientY,
          },
        });
        deps.setContextMenu({ x: event.clientX, y: event.clientY, visible: true });
      } else {
        cancelCanvasInteraction(graphPath, groupId);
      }
      clearCanvasPointerScope();
      return;
    }

    if (interaction.type === "panning") {
      if (!interaction.session.moved && event.button === 2) {
        deps.setContextMenu({ x: event.clientX, y: event.clientY, visible: true });
      } else if (interaction.session.moved) {
        commitViewport(editorViewportScope(interaction.session.groupId, graphPath));
        deps.persistViewport({ groupId: interaction.session.groupId, graphPath });
      }
      useGestureStore.getState().clearGesture(interaction.session.moved);
      cancelCanvasInteraction(graphPath, groupId);
    } else if (interaction.type === "selecting") {
      const session = interaction.session;
      if (!selectionScopeIsValid(deps, scope, session)) {
        cancelCanvasInteraction(graphPath, groupId);
        return;
      }
      const ids = selectionPreviewIds.get(scopeKey(scope)) ?? [];
      const moved =
        Math.abs(session.currentX - session.startX) > CONTEXT_MENU_MOVE_THRESHOLD_PX ||
        Math.abs(session.currentY - session.startY) > CONTEXT_MENU_MOVE_THRESHOLD_PX;
      if (moved) {
        deps.setSelectedNodeIds(ids, session.groupId);
      } else {
        deps.setSelectedNodeIds([...session.baseNodeIds], session.groupId);
      }
      cancelCanvasInteraction(graphPath, groupId);
    } else if (interaction.type === "draggingNodes") {
      const session = interaction.session;
      if (session.moved) {
        const overrides = store.positionOverrides[graphPath] ?? {};
        const positions = session.nodeIds.flatMap((nodeId) =>
          overrides[nodeId] ? [{ nodeId, position: overrides[nodeId] }] : [],
        );
        if (positions.length > 0)
          void Promise.resolve(executeCommand(graphPath, "MoveNodes", { positions }))
            .catch(() =>
              logger.graph.warn(
                `MoveNodes command failed graphPath=${graphPath}`,
                "CanvasInteraction",
              ),
            )
            .finally(() => store.clearPositionOverrides(graphPath, session.nodeIds));
      }
      store.finishInteraction(graphPath, groupId);
    }
    clearCanvasPointerScope();
  };

  const onCancel = (event: PointerEvent) => {
    const active = activeInteraction(event);
    if (!active) return;
    if (rAFId !== null) {
      cancelAnimationFrame(rAFId);
      rAFId = null;
    }
    latestEvent = null;
    const [scope] = active;
    cancelCanvasInteraction(scope.graphPath, scope.groupId);
  };

  const cleanupPointerMove = addGlobalEventListener(window, "pointermove", onMove);
  const cleanupPointerUp = addGlobalEventListener(window, "pointerup", onUp);
  const cleanupPointerCancel = addGlobalEventListener(window, "pointercancel", onCancel);
  return () => {
    const scope = getCanvasPointerScope();
    if (scope) cancelCanvasInteraction(scope.graphPath, scope.groupId);
    cleanupPointerMove();
    cleanupPointerUp();
    cleanupPointerCancel();
    if (rAFId !== null) cancelAnimationFrame(rAFId);
    latestEvent = null;
    for (const unregister of selectionCleanupRegistrations.values()) unregister();
    selectionCleanupRegistrations.clear();
    clearCanvasPointerScope();
  };
}

export function attachCanvasPointerLoop(deps: CanvasPointerLoopDeps): () => void {
  depsRef.current = deps;
  attachCount += 1;
  if (attachCount === 1) removeWindowListeners = installPointerLoop();
  return () => {
    attachCount -= 1;
    if (attachCount === 0 && removeWindowListeners) {
      removeWindowListeners();
      removeWindowListeners = null;
      depsRef.current = null;
    }
  };
}
