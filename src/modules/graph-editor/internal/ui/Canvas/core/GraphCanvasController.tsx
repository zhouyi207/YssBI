import { useCallback, useMemo, useRef } from "react";
import {
  useCanvasDrop,
  useCanvasOverlayHandlers,
  useCanvasViewport,
  useCanvasWheelZoom,
  useEditorCanvas,
  type EditorCanvasScope,
  type GraphContextMenuActions,
} from "@/features/application/editor";
import { useNodeDragPreview, useSelectionBoxPreview } from "@/features/core/canvas";
import { useExecutionVisualBinder } from "@/features/core/execution";
import { useGraphRead } from "@/features/core/graph/read";
import { useGraphInteractionUi } from "@/features/core/graphInteraction/ui";
import { editorViewportScope } from "@/features/core/viewport/viewportScope";
import type { NodeCreationDescriptor } from "@/features/domain/nodeCatalog/creationDescriptor";
import type { PinData } from "@/features/domain/editorProjection/graphRuntimeTypes";
import { GraphNodeController } from "../../Nodes/GraphNodeController";
import type { NodePaletteCatalogRowRenderer } from "../../NodePalette";
import CanvasOverlays, { type CanvasOverlaysModel } from "../overlays/CanvasOverlays";
import { ConnectionLine } from "./ConnectionLine";
import { EdgesOverlay } from "./EdgesOverlay";
import { GraphCanvasView } from "./GraphCanvasView";
import { TransformContainer } from "./TransformContainer";
import { ViewportGrid } from "./ViewportGrid";

const EMPTY_NODE_IDS: string[] = [];

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
  const {
    commands: {
      copyNodes,
      cutNodes,
      duplicateNodes,
      deleteNodesById,
      breakAllNodeLinks,
      breakConnectionsById,
      selectLinkedNodes,
      disconnectPinById,
      resetPinValue,
      setSelectedNodeIds,
      setSelectedConnectionIds,
      executeGraph,
      cancelGraphExecution,
      clearGraphArtifacts,
      createNode,
    },
    workspace: { activeGraph, selectedNodeIds, selectedConnectionIds },
    resources: { variables },
    interaction: {
      onCanvasPointerDown,
      onNodePointerDown,
      onPinPointerDown,
      contextMenu,
      setContextMenu,
      pendingConnection,
      setPendingConnection,
      insertRerouteAtConnection,
    },
  } = useEditorCanvas({ mode, scope });
  const activeResourceRef = activeGraph?.graphPath ?? null;
  const gesturePinData = useGraphInteractionUi((state) => {
    if (!interactive || !activeResourceRef) return null;
    const interaction = state.interactions[activeResourceRef];
    if (!interaction || interaction.type === "idle" || interaction.session.groupId !== groupId) {
      return null;
    }
    return interaction.type === "drawingConnection" || interaction.type === "movingConnections"
      ? interaction.session.source
      : null;
  });
  const canvasElementRef = useRef<HTMLDivElement>(null);
  const selectionBoxRef = useRef<HTMLDivElement>(null);
  const viewportScope = useMemo(
    () => (activeResourceRef ? editorViewportScope(groupId, activeResourceRef) : null),
    [activeResourceRef, groupId],
  );

  useNodeDragPreview(
    canvasElementRef,
    interactive ? groupId : null,
    interactive ? activeResourceRef : null,
  );
  useSelectionBoxPreview(
    selectionBoxRef,
    canvasElementRef,
    interactive ? (activeResourceRef ?? undefined) : undefined,
    interactive ? groupId : undefined,
  );
  useExecutionVisualBinder(
    canvasElementRef,
    interactive ? (activeResourceRef ?? undefined) : undefined,
  );

  const selectedNodeIdsSet = useMemo(() => new Set(selectedNodeIds), [selectedNodeIds]);
  const graphNodeIds = useGraphRead((snapshot) =>
    activeResourceRef
      ? (snapshot.graphEntities[activeResourceRef]?.graphNodes ?? EMPTY_NODE_IDS)
      : EMPTY_NODE_IDS,
  );
  const { getPinWorldPos, getCanvasLocalPoint } = useCanvasViewport(
    canvasElementRef,
    groupId,
    activeResourceRef,
  );
  useCanvasWheelZoom(canvasElementRef, viewportScope, interactive);
  const {
    variableDropMenu,
    setVariableDropMenu,
    handleNodeAddInput,
    handleNodeRemovePin,
    handleContextMenu,
    handleVariableDropGet,
    handleVariableDropSet,
  } = useCanvasDrop({
    canvasElementRef,
    panelInstanceId,
    groupId,
    graphPath: activeResourceRef,
    variables,
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
  const activePin = useMemo<PinData | null>(() => {
    if (!interactive) return null;
    if (gesturePinData) return structuredClone(gesturePinData) as PinData;
    if (pendingConnection && contextMenu?.visible) return pendingConnection;
    return null;
  }, [contextMenu, gesturePinData, interactive, pendingConnection]);
  const sourcePort = pendingConnection?.address ?? null;
  const handlePaletteSelect = useCallback(
    (descriptor: NodeCreationDescriptor, locale: string) => {
      if (contextMenu?.visible) {
        void handleNodePaletteSelect(descriptor, locale, contextMenu);
      }
    },
    [contextMenu, handleNodePaletteSelect],
  );
  const handleSelectedConnectionIdsChange = useCallback(
    (connectionIds: string[], targetGraphPath: string, targetGroupId: string) => {
      if (targetGraphPath === activeResourceRef && targetGroupId === groupId) {
        setSelectedConnectionIds(connectionIds, targetGroupId);
      }
    },
    [activeResourceRef, groupId, setSelectedConnectionIds],
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
      variable: variableDropMenu
        ? {
            kind: "visible",
            x: variableDropMenu.x,
            y: variableDropMenu.y,
            variableName: variableDropMenu.variableName,
            onGet: () => {
              void handleVariableDropGet(variableDropMenu);
            },
            onSet: () => {
              void handleVariableDropSet(variableDropMenu);
            },
            onClose: () => setVariableDropMenu(null),
          }
        : { kind: "hidden" },
      execution:
        activeGraph?.kind === "event"
          ? {
              kind: "event",
              graphPath: activeGraph.graphPath,
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
      contextMenu,
      executeGraph,
      handlePaletteSelect,
      handleVariableDropGet,
      handleVariableDropSet,
      setVariableDropMenu,
      sourcePort,
      variableDropMenu,
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
      removeRepeatablePin: handleNodeRemovePin,
    }),
    [
      breakAllNodeLinks,
      copyNodes,
      cutNodes,
      deleteNodesById,
      disconnectPinById,
      duplicateNodes,
      groupId,
      handleNodeRemovePin,
      resetPinValue,
      selectLinkedNodes,
      setSelectedNodeIds,
    ],
  );
  const isDraggingPin = activePin != null;
  const connectionPreviewSlot = (
    <ConnectionLine
      viewportScope={viewportScope}
      getPinWorldPos={getPinWorldPos}
      getCanvasLocalPoint={getCanvasLocalPoint}
      pendingConnection={interactive ? pendingConnection : null}
      menuPos={interactive ? contextMenu : null}
    />
  );
  const graphContentSlot = (
    <TransformContainer viewportScope={viewportScope}>
      {interactive ? (
        <EdgesOverlay
          mode="interactive"
          graphPath={activeResourceRef ?? ""}
          groupId={groupId}
          getPinWorldPos={getPinWorldPos}
          getCanvasLocalPoint={getCanvasLocalPoint}
          dimmed={isDraggingPin}
          selectedNodeIds={selectedNodeIds}
          selectedConnectionIds={selectedConnectionIds}
          onSelectedConnectionIdsChange={handleSelectedConnectionIdsChange}
          onBreakConnections={breakConnectionsById}
          onEdgeDoubleClick={insertRerouteAtConnection}
        />
      ) : (
        <EdgesOverlay
          mode="preview"
          graphPath={activeResourceRef ?? ""}
          groupId={groupId}
          getPinWorldPos={getPinWorldPos}
          getCanvasLocalPoint={getCanvasLocalPoint}
          dimmed={isDraggingPin}
        />
      )}
      {graphNodeIds.map((nodeId) => (
        <GraphNodeController
          key={nodeId}
          id={nodeId}
          graphPath={activeResourceRef ?? undefined}
          groupId={groupId}
          selected={interactive && selectedNodeIdsSet.has(nodeId)}
          activePin={activePin}
          contextMenuActions={interactive ? contextMenuActions : null}
          onPointerDown={onNodePointerDown}
          onAddInput={handleNodeAddInput}
          onRemovePin={handleNodeRemovePin}
          onPinPointerDown={onPinPointerDown}
        />
      ))}
    </TransformContainer>
  );

  return (
    <GraphCanvasView
      canvasElementRef={canvasElementRef}
      selectionBoxRef={selectionBoxRef}
      panelInstanceId={panelInstanceId}
      graphPath={activeResourceRef ?? undefined}
      graphKind={graphKind}
      viewportGridSlot={<ViewportGrid viewportScope={viewportScope} />}
      connectionPreviewSlot={connectionPreviewSlot}
      graphContentSlot={graphContentSlot}
      overlaySlot={
        interactive ? (
          <CanvasOverlays model={overlayModel} catalogRowRenderer={catalogRowRenderer} />
        ) : null
      }
      onCanvasPointerDown={onCanvasPointerDown}
      onCanvasContextMenu={interactive ? handleContextMenu : undefined}
    />
  );
}
