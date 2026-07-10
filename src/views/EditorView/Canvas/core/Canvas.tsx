import { useRef, useMemo, useCallback, useEffect } from "react";
import { useShallow } from "zustand/react/shallow";
import { CanvasNode } from "../../Nodes/CanvasNode";
import { useGraphDataStore } from "@/features/core/dataStore";
import { useEditorGroup, useCanvasViewport, useCanvasWheelZoom, useCanvasDrop } from "@/features/application/editor";
import { CanvasContextMenuProvider } from "@/features/application/editor/CanvasContextMenuContext";
import type { CanvasContextMenuActions } from "@/features/application/editor/CanvasContextMenuContext";
import { useGestureStore } from "@/features/core/gesture";
import { bindDragPreviewToGestureStore } from "@/features/core/canvas/dragPreview";
import { getConnectGesture, type EditorGesture } from "@/shared/types/ui";
import { useNodeDragPreview } from "@/features/core/canvas/useNodeDragPreview";
import { useSelectionBoxPreview } from "@/features/core/canvas/useSelectionBoxPreview";
import { useExecutionVisualBinder } from "@/features/core/execution";
import { ViewportGrid } from "./ViewportGrid";
import { TransformContainer } from "./TransformContainer";
import { EdgesOverlay } from "./EdgesOverlay";
import { ConnectionLine } from "./ConnectionLine";
import CanvasOverlays from "../overlays/CanvasOverlays";

const selectGestureType = (state: { gesture: EditorGesture }) => state.gesture?.type ?? null;
const selectActivePin = (state: { gesture: EditorGesture }) =>
  getConnectGesture(state.gesture)?.startPin ?? null;

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
    copyNodes,
    cutNodes,
    duplicateNodes,
    deleteNodesById,
    breakAllNodeLinks,
    selectLinkedNodes,
    disconnectPinById,
    resetPinValue,
    setSelectedNodeIds,
    createNode,
  } = useEditorGroup({ withCanvasInteraction: interactive });

  const gestureType = useGestureStore(selectGestureType);
  const gesturePinData = useGestureStore(selectActivePin);

  const canvasElementRef = useRef<HTMLDivElement>(null);
  const selectionBoxRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!interactive) return;
    return bindDragPreviewToGestureStore();
  }, [interactive]);

  useNodeDragPreview(canvasElementRef, interactive ? activeTabId : null);
  useSelectionBoxPreview(selectionBoxRef, canvasElementRef, interactive ? groupId : undefined);
  useExecutionVisualBinder(canvasElementRef, interactive ? activeTabId ?? undefined : undefined);

  const selectedNodeIdsSet = useMemo(
    () => new Set(selectedNodeIds),
    [selectedNodeIds]
  );

  const graphNodeIds = useGraphDataStore(
    useShallow((s) => (activeTabId ? s.getGraphNodeIds(activeTabId) : EMPTY_NODE_IDS)),
  );

  const { visibleNodeIds, getPinWorldPos, getCanvasLocalPoint } = useCanvasViewport(
    canvasElementRef,
    activeTabId,
    interactive ? gestureType : null,
  );
  useCanvasWheelZoom(canvasElementRef, interactive ? activeTabId : null);

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

  const handlePinClick = useCallback(() => {}, []);
  const handlePinValueChange = useCallback(() => {}, []);

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
      <ViewportGrid graphPath={activeTabId ?? ""} />

      <div
        className="absolute inset-0"
        onPointerDown={onCanvasPointerDown}
        onContextMenu={interactive ? handleContextMenu : undefined}
      >
        <ConnectionLine
          graphPath={activeTabId ?? ""}
          getPinWorldPos={getPinWorldPos}
          getCanvasLocalPoint={getCanvasLocalPoint}
          pendingConnection={interactive ? pendingConnection : null}
          menuPos={interactive ? contextMenu : null}
        />

        <TransformContainer graphPath={activeTabId ?? ""}>
          <EdgesOverlay
            graphPath={activeTabId ?? ""}
            getPinWorldPos={getPinWorldPos}
            dimmed={isDraggingPin}
          />
          {graphNodeIds.map((nodeId: string) => {
            if (!visibleNodeIds.has(nodeId)) return null;
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
                onPinClick={handlePinClick}
                onPinPointerDown={onPinPointerDown}
                onPinValueChange={handlePinValueChange}
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
