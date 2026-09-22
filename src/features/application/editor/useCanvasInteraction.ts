import { useCallback, useEffect, useRef } from "react";
import {
  getEditorGroupGraphSelection,
  updateEditorGroupSelectedConnectionIds,
  type GraphSelection,
} from "@/modules/workbench/public";
import { useEditorStore } from "@/features/core/editor";
import {
  getCanvasInteraction,
  useGraphInteractionStore,
  type CanvasGestureType,
} from "@/features/core/graphInteraction/graphInteractionStore";
import {
  cancelCanvasInteraction,
  registerCanvasInteractionCleanup,
  startCanvasInteraction,
} from "@/features/core/canvas/canvasInteractionCleanup";
import type {
  CanvasGestureLease,
  CanvasInteractionHandlers,
} from "@/features/core/canvas/canvasMutationContracts";
import { isGraphSaving } from "@/features/core/dataStore/graphProjectionStore";
import type { PortAddressDto } from "@/shared/types/domain/editorProjection";
import { captureEditorCommandTarget, isEditorCommandTargetCurrent } from "./editorCommandFocus";
import { activateCurrentEditorPanel } from "./activateEditorPanelAndSyncSession";
import type { EditorCanvasScope } from "./editorCanvasTypes";

function selectionMatches(actual: GraphSelection, expected: GraphSelection): boolean {
  return (
    actual.nodeIds.size === expected.nodeIds.size &&
    [...actual.nodeIds].every((id) => expected.nodeIds.has(id)) &&
    actual.connectionIds.size === expected.connectionIds.size &&
    [...actual.connectionIds].every((id) => expected.connectionIds.has(id))
  );
}

export function useCanvasInteraction({
  scope,
  enabled,
  handlers,
  setSelectedNodeIds,
}: {
  scope: EditorCanvasScope;
  enabled: boolean;
  handlers: CanvasInteractionHandlers;
  setSelectedNodeIds: (ids: string[], groupId?: string) => void;
}) {
  const { graphPath, groupId, panelInstanceId } = scope;
  const enabledRef = useRef(enabled);
  enabledRef.current = enabled;
  const contextMenu = useEditorStore((state) => {
    const menu = state.contextMenu;
    return enabled && menu?.panelInstanceId === panelInstanceId && menu.graphPath === graphPath
      ? menu
      : null;
  });
  const pendingConnection = useGraphInteractionStore((state) => {
    const interaction = getCanvasInteraction(state, graphPath, groupId);
    return enabled &&
      interaction.type === "pendingNodeCreation" &&
      interaction.session.panelInstanceId === panelInstanceId
      ? interaction.session.source
      : null;
  });
  const captureTarget = useCallback(() => {
    if (!enabledRef.current || isGraphSaving(graphPath)) return null;
    const target = captureEditorCommandTarget(panelInstanceId);
    return target?.panelInstanceId === panelInstanceId &&
      target.groupId === groupId &&
      target.resourceRef === graphPath &&
      isEditorCommandTargetCurrent(target)
      ? target
      : null;
  }, [graphPath, groupId, panelInstanceId]);
  const isInteractive = useCallback(() => captureTarget() !== null, [captureTarget]);
  const cancelOwnedInteraction = useCallback(() => {
    const current = getCanvasInteraction(useGraphInteractionStore.getState(), graphPath, groupId);
    if (current.type !== "idle" && current.session.panelInstanceId === panelInstanceId) {
      cancelCanvasInteraction(graphPath, groupId);
    }
  }, [graphPath, groupId, panelInstanceId]);
  useEffect(() => {
    if (!enabled) cancelOwnedInteraction();
    return cancelOwnedInteraction;
  }, [enabled, cancelOwnedInteraction]);

  const beginGesture = useCallback(
    (type: CanvasGestureType, onCancel: () => void): CanvasGestureLease | null => {
      const target = captureTarget();
      if (!target) return null;
      activateCurrentEditorPanel(groupId);
      if (!isEditorCommandTargetCurrent(target)) return null;
      startCanvasInteraction(graphPath, { type, session: { groupId, panelInstanceId } });
      const owner = useGraphInteractionStore.getState().interactions[graphPath];
      let active = true;
      const unregister = registerCanvasInteractionCleanup(
        { graphPath, groupId, interactionType: type },
        () => {
          active = false;
          onCancel();
        },
      );
      return {
        isCurrent: () =>
          active &&
          enabledRef.current &&
          !isGraphSaving(graphPath) &&
          isEditorCommandTargetCurrent(target) &&
          useGraphInteractionStore.getState().interactions[graphPath] === owner,
        finish: () => {
          active = false;
          unregister();
          if (useGraphInteractionStore.getState().interactions[graphPath] === owner) {
            useGraphInteractionStore.getState().finishInteraction(graphPath, groupId);
          }
        },
      };
    },
    [captureTarget, graphPath, groupId, panelInstanceId],
  );

  const setPendingConnection = useCallback(
    (port: PortAddressDto | null) => {
      if (!port) {
        const current = getCanvasInteraction(
          useGraphInteractionStore.getState(),
          graphPath,
          groupId,
        );
        if (current.type === "pendingNodeCreation") cancelOwnedInteraction();
        return;
      }
      if (!captureTarget()) return;
      startCanvasInteraction(graphPath, {
        type: "pendingNodeCreation",
        session: {
          groupId,
          panelInstanceId,
          source: port,
        },
      });
    },
    [captureTarget, cancelOwnedInteraction, graphPath, groupId, panelInstanceId],
  );

  const insertRerouteAtConnection = useCallback(
    async (
      connectionId: string,
      position: Readonly<{ x: number; y: number }>,
      targetGraphPath: string,
      targetGroupId: string,
      selection: { before: GraphSelection; temporary: GraphSelection },
    ) => {
      const target = captureTarget();
      if (!target || targetGraphPath !== graphPath || targetGroupId !== groupId)
        return false as const;
      const outcome = await handlers
        .insertRerouteAtConnection({ graphPath, connectionId, position })
        .catch(() => false as const);
      if (
        !isEditorCommandTargetCurrent(target) ||
        !selectionMatches(getEditorGroupGraphSelection(groupId), selection.temporary)
      )
        return outcome;
      if (outcome !== false && outcome.status === "applied") {
        updateEditorGroupSelectedConnectionIds(
          [...selection.temporary.connectionIds].filter((id) => id !== connectionId),
          groupId,
        );
      } else if (selection.before.nodeIds.size) {
        setSelectedNodeIds([...selection.before.nodeIds], groupId);
      } else {
        updateEditorGroupSelectedConnectionIds([...selection.before.connectionIds], groupId);
      }
      return outcome;
    },
    [captureTarget, graphPath, groupId, handlers, setSelectedNodeIds],
  );

  return {
    contextMenu,
    pendingConnection,
    setPendingConnection,
    beginGesture,
    isInteractive,
    mutations: handlers,
    insertRerouteAtConnection,
  };
}
