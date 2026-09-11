import { useCallback, useMemo, useRef } from "react";
import {
  useCanvasDrop,
  useCanvasOverlayHandlers,
  useCanvasViewport,
  useEditorCanvas,
  type EditorCanvasScope,
  type GraphContextMenuActions,
} from "@/features/application/editor";
import { isGraphProjectionExecutable } from "@/features/core/dataStore/graphEntityAccess";
import { useExecutionVisualBinder } from "@/features/core/execution";
import { useGraphRead } from "@/features/core/graph/read";
import { editorViewportScope } from "@/features/core/viewport/viewportScope";
import type { NodeCreationDescriptor } from "@/features/domain/nodeCatalog/creationDescriptor";
import type { NodePaletteCatalogRowRenderer } from "../../NodePalette";
import CanvasOverlays, { type CanvasOverlaysModel } from "../overlays/CanvasOverlays";
import { GraphCanvasView } from "./GraphCanvasView";
import { GraphFlowCanvas } from "./GraphFlowCanvas";
import { ViewportGrid } from "./ViewportGrid";

export interface GraphCanvasControllerProps {
  mode: "interactive" | "preview";
  panelInstanceId: string;
  groupId: string;
  graphPath: string;
  graphKind: "event" | "function";
  catalogRowRenderer: NodePaletteCatalogRowRenderer;
}

export function GraphCanvasController({
  mode,
  panelInstanceId,
  groupId,
  graphPath,
  graphKind,
  catalogRowRenderer,
}: GraphCanvasControllerProps) {
  const interactive = mode === "interactive";
  const scope = useMemo<EditorCanvasScope>(
    () => ({ panelInstanceId, groupId, graphPath, graphKind }),
    [graphKind, graphPath, groupId, panelInstanceId],
  );
  const canvas = useEditorCanvas({ mode, scope });
  const {
    commands: {
      copyNodes,
      cutNodes,
      duplicateNodes,
      deleteNodesById,
      breakAllNodeLinks,
      selectLinkedNodes,
      disconnectPinById,
      resetPinValue,
      setSelectedNodeIds,
      compileGraph,
      executeGraph,
      cancelGraphExecution,
      clearGraphArtifacts,
      createNode,
    },
    workspace: { activeGraph, compileStatus },
    interaction: { contextMenu, setContextMenu, pendingConnection, setPendingConnection },
  } = canvas;
  const activeResourceRef = activeGraph?.graphPath ?? null;
  const canvasElementRef = useRef<HTMLDivElement>(null);
  const viewportScope = useMemo(
    () => (activeResourceRef ? editorViewportScope(groupId, activeResourceRef) : null),
    [activeResourceRef, groupId],
  );

  useExecutionVisualBinder(
    canvasElementRef,
    interactive ? (activeResourceRef ?? undefined) : undefined,
  );

  const projectionAllowsExecution = useGraphRead((snapshot) =>
    activeResourceRef
      ? isGraphProjectionExecutable(snapshot.graphEntities[activeResourceRef])
      : false,
  );
  const { viewport, setViewport, commit } = useCanvasViewport(groupId, graphPath);
  const { handleContextMenu } = useCanvasDrop({
    canvasElementRef,
    panelInstanceId,
    groupId,
    graphPath: activeResourceRef,
    setContextMenu,
    setPendingConnection,
    createNode,
    enabled: interactive,
  });
  const { handleNodePaletteSelect } = useCanvasOverlayHandlers({
    canvasElementRef,
    panelInstanceId,
    groupId,
    activeResourceRef,
    pendingConnection,
    setContextMenu,
    setPendingConnection,
  });
  const sourcePort = pendingConnection?.address ?? null;
  const handlePaletteSelect = useCallback(
    (descriptor: NodeCreationDescriptor, locale: string) => {
      if (contextMenu?.visible) {
        void handleNodePaletteSelect(descriptor, locale, contextMenu);
      }
    },
    [contextMenu, handleNodePaletteSelect],
  );
  const closePalette = useCallback(() => {
    setContextMenu(null);
    setPendingConnection(null);
  }, [setContextMenu, setPendingConnection]);
  const overlayModel = useMemo(
    (): CanvasOverlaysModel => ({
      graph: activeGraph
        ? { kind: activeGraph.kind, graphPath: activeGraph.graphPath }
        : { kind: "unavailable" },
      palette: contextMenu?.visible
        ? {
            kind: "visible",
            x: contextMenu.x,
            y: contextMenu.y,
            graphPath: activeResourceRef,
            sourcePort,
            onSelect: handlePaletteSelect,
            onClose: closePalette,
          }
        : { kind: "hidden" },
      execution: activeGraph
        ? {
            kind: "graph",
            graphPath: activeGraph.graphPath,
            canExecute: activeGraph.kind === "event" && projectionAllowsExecution,
            executeUnavailableReason:
              activeGraph.kind === "function"
                ? "functionGraph"
                : projectionAllowsExecution
                  ? null
                  : "blockingProblems",
            compileStatus,
            onCompile: () => {
              void compileGraph(activeGraph.graphPath);
            },
            onExecute: () => {
              void executeGraph(activeGraph.graphPath);
            },
            onCancelExecution: () => {
              void cancelGraphExecution(activeGraph.graphPath);
            },
            onClearArtifacts: () => {
              void clearGraphArtifacts(activeGraph.graphPath);
            },
          }
        : { kind: "hidden" },
    }),
    [
      activeGraph,
      activeResourceRef,
      cancelGraphExecution,
      clearGraphArtifacts,
      closePalette,
      compileGraph,
      contextMenu,
      executeGraph,
      handlePaletteSelect,
      projectionAllowsExecution,
      sourcePort,
      compileStatus,
    ],
  );
  const contextMenuActions = useMemo(
    (): GraphContextMenuActions => ({
      selectNode: (nodeId, targetGroupId) => setSelectedNodeIds([nodeId], targetGroupId ?? groupId),
      copyNode: (nodeId) => copyNodes([nodeId]),
      cutNode: (nodeId) => cutNodes([nodeId]),
      duplicateNode: (nodeId) => duplicateNodes([nodeId]),
      deleteNode: (nodeId) => deleteNodesById([nodeId]),
      breakAllNodeLinks,
      selectLinkedNodes,
      disconnectPin: disconnectPinById,
      resetPinValue,
    }),
    [
      breakAllNodeLinks,
      copyNodes,
      cutNodes,
      deleteNodesById,
      disconnectPinById,
      duplicateNodes,
      groupId,
      resetPinValue,
      selectLinkedNodes,
      setSelectedNodeIds,
    ],
  );
  const graphContentSlot = (
    <GraphFlowCanvas
      panelInstanceId={panelInstanceId}
      graphPath={graphPath}
      groupId={groupId}
      interactive={interactive}
      canvas={canvas}
      contextMenuActions={interactive ? contextMenuActions : null}
      viewport={viewport}
      onViewportChange={setViewport}
      onViewportCommit={commit}
      onContextMenu={handleContextMenu}
    />
  );

  return (
    <GraphCanvasView
      canvasElementRef={canvasElementRef}
      panelInstanceId={panelInstanceId}
      graphPath={activeResourceRef ?? undefined}
      graphKind={graphKind}
      viewportGridSlot={<ViewportGrid viewportScope={viewportScope} />}
      graphContentSlot={graphContentSlot}
      overlaySlot={
        interactive ? (
          <CanvasOverlays model={overlayModel} catalogRowRenderer={catalogRowRenderer} />
        ) : null
      }
    />
  );
}
