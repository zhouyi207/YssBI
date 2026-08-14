import { useRef, useMemo } from "react";
import { useShallow } from "zustand/react/shallow";
import { CanvasNode } from "../../Nodes/CanvasNode";
import { useGraphDataStore } from "@/features/core/dataStore";
import { useEditorGroup, useCanvasViewport, useCanvasWheelZoom, useCanvasDrop } from "@/features/application/editor";
import { editorViewportScope } from "@/features/core/viewport";
import { CanvasContextMenuProvider } from "@/features/application/editor/CanvasContextMenuContext";
import type { CanvasContextMenuActions } from "@/features/application/editor/CanvasContextMenuContext";
import { getCanvasInteraction, useGraphInteractionStore } from '@/features/core/graphInteraction/graphInteractionStore';
import { useNodeDragPreview } from "@/features/core/canvas/useNodeDragPreview";
import { useSelectionBoxPreview } from "@/features/core/canvas/useSelectionBoxPreview";
import { useExecutionVisualBinder } from "@/features/core/execution";
import { ViewportGrid } from "./ViewportGrid";
import { TransformContainer } from "./TransformContainer";
import { EdgesOverlay } from "./EdgesOverlay";
import { ConnectionLine } from "./ConnectionLine";
import CanvasOverlays from "../overlays/CanvasOverlays";


const EMPTY_NODE_IDS: string[] = [];

export type CanvasProps = {
  /** Interactive editing (active group). Preview mode keeps the graph visible without side effects. */
  interactive?: boolean;
};

export default function Canvas({ interactive = true }: CanvasProps) {
  const {
    onCanvasPointerDown,
    onNodePointerDown,
    onPinPointerDown,
    contextMenu,
    setContextMenu,
    variables,
    activeTabId,
    pendingConnection,
    setPendingConnection,
    functions,
    groupId,
    selectedNodeIds,
    selectedConnectionIds,
    copyNodes,
    cutNodes,
    duplicateNodes,
    deleteNodesById,
    breakAllNodeLinks,
    breakConnectionsById,
    insertRerouteAtConnection,
    selectLinkedNodes,
    disconnectPinById,
    resetPinValue,
    setSelectedNodeIds,
    setSelectedConnectionIds,
    createNode,
  } = useEditorGroup({ withCanvasPointerLoop: interactive });

  const gesturePinData = useGraphInteractionStore((state) => {
    if (!activeTabId) return null;
    const interaction = getCanvasInteraction(state, activeTabId, groupId);
    return interaction.type === 'drawingConnection' || interaction.type === 'movingConnections'
      ? interaction.session.source
      : null;
  });

  const canvasElementRef = useRef<HTMLDivElement>(null);
  const selectionBoxRef = useRef<HTMLDivElement>(null);


  const viewportScope = useMemo(
    () => (groupId && activeTabId ? editorViewportScope(groupId, activeTabId) : null),
    [groupId, activeTabId],
  );

  useNodeDragPreview(canvasElementRef, interactive ? groupId : null, interactive ? activeTabId : null);
  useSelectionBoxPreview(
    selectionBoxRef,
    canvasElementRef,
    interactive ? activeTabId ?? undefined : undefined,
    interactive ? groupId : undefined,
  );
  useExecutionVisualBinder(canvasElementRef, interactive ? activeTabId ?? undefined : undefined);

  const selectedNodeIdsSet = useMemo(
    () => new Set(selectedNodeIds),
    [selectedNodeIds]
  );

  const graphNodeIds = useGraphDataStore(
    useShallow((s) => (activeTabId ? s.getGraphNodeIds(activeTabId) : EMPTY_NODE_IDS)),
  );

  const { getPinWorldPos, getCanvasLocalPoint } = useCanvasViewport(
    canvasElementRef,
    groupId,
    activeTabId,
  );
  useCanvasWheelZoom(canvasElementRef, viewportScope);

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
    groupId,
    graphPath: interactive ? activeTabId : null,
    variables,
    functions,
    setContextMenu,
    setPendingConnection,
    createNode,
    enabled: interactive,
  });

  const activePin = useMemo(() => {
    if (!interactive) return null;
    if (gesturePinData) return gesturePinData;
    if (pendingConnection && contextMenu?.visible) return pendingConnection;
    return null;
  }, [interactive, gesturePinData, pendingConnection, contextMenu]);

  const isDraggingPin = activePin != null;

  const contextMenuActions = useMemo((): CanvasContextMenuActions => ({
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
  }), [
    groupId,
    setSelectedNodeIds,
    copyNodes,
    cutNodes,
    duplicateNodes,
    deleteNodesById,
    breakAllNodeLinks,
    selectLinkedNodes,
    disconnectPinById,
    resetPinValue,
    handleNodeRemovePin,
  ]);

  return (
    <CanvasContextMenuProvider value={contextMenuActions}>
    <div
      ref={canvasElementRef}
      data-editor-group-id={groupId}
      className="relative w-full h-full overflow-hidden bg-[var(--workbench-bg)] select-none"
    >
      <ViewportGrid viewportScope={viewportScope} />

      <div
        className="absolute inset-0"
        onPointerDown={onCanvasPointerDown}
        onContextMenu={interactive ? handleContextMenu : undefined}
      >
        <ConnectionLine
          viewportScope={viewportScope}
          getPinWorldPos={getPinWorldPos}
          getCanvasLocalPoint={getCanvasLocalPoint}
          pendingConnection={interactive ? pendingConnection : null}
          menuPos={interactive ? contextMenu : null}
        />

        <TransformContainer viewportScope={viewportScope}>
          <EdgesOverlay
            graphPath={activeTabId ?? ""}
            groupId={groupId}
            getPinWorldPos={getPinWorldPos}
            getCanvasLocalPoint={getCanvasLocalPoint}
            dimmed={isDraggingPin}
            interactive={interactive}
            selectedNodeIds={interactive ? selectedNodeIds : EMPTY_NODE_IDS}
            selectedConnectionIds={interactive ? selectedConnectionIds : EMPTY_NODE_IDS}
            onSelectedConnectionIdsChange={interactive
              ? (connectionIds, graphPath, targetGroupId) => {
                  if (graphPath === activeTabId && targetGroupId === groupId) {
                    setSelectedConnectionIds(connectionIds, targetGroupId);
                  }
                }
              : undefined}
            onBreakConnections={interactive ? breakConnectionsById : undefined}
            onEdgeDoubleClick={interactive ? insertRerouteAtConnection : undefined}
          />
          {graphNodeIds.map((nodeId: string) => {
            const isSelected = interactive && selectedNodeIdsSet.has(nodeId);
            return (
              <CanvasNode
                key={nodeId}
                id={nodeId}
                graphPath={activeTabId || undefined}
                groupId={groupId}
                selected={isSelected}
                activePin={activePin}
                onPointerDown={onNodePointerDown}
                onAddInput={handleNodeAddInput}
                onRemovePin={handleNodeRemovePin}
                onPinPointerDown={onPinPointerDown}
              />
            );
          })}
        </TransformContainer>
      </div>

      <div ref={selectionBoxRef} aria-hidden />

      {interactive ? (
        <CanvasOverlays
          canvasElementRef={canvasElementRef}
          variableDropMenu={variableDropMenu}
          setVariableDropMenu={setVariableDropMenu}
          onVariableDropGet={handleVariableDropGet}
          onVariableDropSet={handleVariableDropSet}
        />
      ) : null}
    </div>
    </CanvasContextMenuProvider>
  );
}
