import { useEffect, useMemo, useRef } from "react";
import { useShallow } from "zustand/react/shallow";
import { useTranslation } from "react-i18next";
import {
  captureActiveEditorCommandTarget,
  isEditorCommandTargetCurrent,
} from "@/features/application/editor/editorCommandFocus";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import { useEditorPaneStateStore } from "@/modules/workbench/public";
import { useDockviewPortSnapshot } from "@/modules/workbench/public";
import { workbenchDockviewRead } from "@/modules/workbench/public";
import {
  getViewport,
  subscribeToViewport,
  editorViewportScope,
  type ViewportScope,
} from "@/features/core/viewport";
import {
  createBuiltInStatusBarItems,
  useStatusBarSnapshot,
  type StatusBarItemsSnapshot,
  type StatusBarRenderContext,
} from "@/features/core/statusBar";
import { useStatusBarActions } from "./useStatusBarActions";

function formatViewportStatus(scope: ViewportScope | null) {
  if (!scope) return "X 0 Y 0 100%";
  const viewport = getViewport(scope);
  return `X ${Math.round(viewport.x)} Y ${Math.round(viewport.y)} ${Math.round(viewport.scale * 100)}%`;
}

function ViewportStatus({ scope }: { scope: ViewportScope | null }) {
  const ref = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    let frame: number | undefined;
    const update = () => {
      frame = undefined;
      const text = formatViewportStatus(scope);
      if (ref.current && ref.current.textContent !== text) ref.current.textContent = text;
    };
    // React may retain its original text after live DOM updates, including on a null scope.
    update();
    if (!scope) return;
    const unsubscribe = subscribeToViewport(scope, () => {
      if (frame === undefined) frame = requestAnimationFrame(update);
    });
    return () => {
      unsubscribe();
      if (frame !== undefined) cancelAnimationFrame(frame);
    };
  }, [scope?.groupId, scope?.graphPath]);

  return (
    <span ref={ref} className="tabular-nums">
      {formatViewportStatus(scope)}
    </span>
  );
}

export function useStatusBarItems(): StatusBarItemsSnapshot {
  const { t } = useTranslation();
  const actions = useStatusBarActions();

  const graphTarget = useDockviewPortSnapshot(
    workbenchDockviewRead,
    useShallow(() => {
      const target = captureActiveEditorCommandTarget();
      return target &&
        isEditorCommandTargetCurrent(target) &&
        (target.resourceKind === "event" || target.resourceKind === "function")
        ? target
        : null;
    }),
  );
  const selectedCount = useEditorPaneStateStore((state) =>
    graphTarget ? (state.selections[graphTarget.panelInstanceId]?.selectedNodeIds.length ?? 0) : 0,
  );
  const graphPath = graphTarget?.resourceRef;
  const nodeCount = useGraphProjectionStore((state) =>
    graphPath ? (state.graphEntities[graphPath]?.graphNodes.length ?? 0) : 0,
  );
  const connections = useGraphProjectionStore((state) =>
    graphPath ? state.graphEntities[graphPath]?.connections : undefined,
  );
  const connectionCount = useMemo(
    () => (connections ? Object.keys(connections).length : 0),
    [connections],
  );

  const ctx = useMemo<StatusBarRenderContext>(
    () => ({
      t,
      activeResourceRef: graphPath ?? null,
      activeEditorGroupId: graphTarget?.groupId ?? null,
      selectedCount,
      nodeCount,
      connectionCount,
    }),
    [t, graphPath, graphTarget?.groupId, selectedCount, nodeCount, connectionCount],
  );

  const builtIn = useMemo(
    () =>
      createBuiltInStatusBarItems({
        resetCanvasViewport: actions.resetCanvasViewport,
        viewportTooltip: actions.viewportTooltip,
        renderViewportStatus: (groupId, graphPath) => (
          <ViewportStatus
            scope={groupId && graphPath ? editorViewportScope(groupId, graphPath) : null}
          />
        ),
      }),
    [actions.resetCanvasViewport, actions.viewportTooltip],
  );

  return useStatusBarSnapshot(ctx, builtIn);
}
