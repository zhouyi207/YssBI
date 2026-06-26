import { useRef, useMemo, useCallback } from "react";
import { useShallow } from "zustand/react/shallow";
import { CanvasNode } from "../../Nodes/CanvasNode";
import { useGraphDataStore } from "@/features/core/dataStore";
import { useEditorGroup, useCanvasViewport, useCanvasDrop } from "@/features/application/editor";
import { CanvasContextMenuProvider } from "@/features/application/editor/CanvasContextMenuContext";
import type { CanvasContextMenuActions } from "@/features/application/editor/CanvasContextMenuContext";
import { useGestureStore } from "@/features/core/gesture";
import { useViewportStore } from "@/features/core/viewport";
import { useNodeManagement } from "@/features/application/dataManagement";
import { ViewportGrid } from "./ViewportGrid";
import { TransformContainer } from "./TransformContainer";
import { EdgesOverlay } from "./EdgesOverlay";
import { ConnectionLine } from "./ConnectionLine";
import CanvasOverlays from "../overlays/CanvasOverlays";

// 粒度化 gesture 选择器：仅在值实际改变时触发 re-render
const selectDragDelta = (state: { gesture: any }) => {
  const g = state.gesture;
  return g?.type === "drag" ? g.dragDelta ?? null : null;
};
const selectDragNodeIds = (state: { gesture: any }) => {
  const g = state.gesture;
  return g?.type === "drag" ? g.dragNodeIds ?? null : null;
};
const selectGestureType = (state: { gesture: any }) => state.gesture?.type ?? null;
const selectActivePin = (state: { gesture: any }) => {
  const g = state.gesture;
  return g?.type === "connect" ? g.startPin : null;
};
const dragDeltaEq = (a: any, b: any) => a?.x === b?.x && a?.y === b?.y;

const EMPTY_NODE_IDS: string[] = [];

export default function Canvas() {
  const {
    setCanvas,
    setNodes,
    onCanvasPointerDown,
    onNodePointerDown,
    onPinPointerDown,
    contextMenu,
    setContextMenu,
    variables,
    Variables,
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
  } = useEditorGroup();

  const dragDelta = useGestureStore(selectDragDelta, dragDeltaEq);
  const dragNodeIds = useGestureStore(selectDragNodeIds);
  const gestureType = useGestureStore(selectGestureType);
  const gesturePinData = useGestureStore(selectActivePin);

  const { createNode } = useNodeManagement();

  const ref = useRef<HTMLDivElement>(null);
  const scale = useViewportStore((state) =>
    activeTabId ? (state.viewports[activeTabId]?.scale ?? 1) : 1,
  );

  const selectedNodeIdsSet = useMemo(
    () => new Set(selectedNodeIds),
    [selectedNodeIds]
  );

  const dragNodeIdsSet = useMemo(
    () => (dragNodeIds ? new Set(dragNodeIds) : new Set<string>()),
    [dragNodeIds]
  );

  // 稳定的节点 id 列表（仅在增删/排序时变化），逐节点订阅渲染。
  const graphNodeIds = useGraphDataStore(
    useShallow((s) => (activeTabId ? s.graphNodes[activeTabId] ?? EMPTY_NODE_IDS : EMPTY_NODE_IDS)),
  );

  const { visibleNodeIds, getPinWorldPos, getCanvasLocalPoint } = useCanvasViewport(
    ref,
    activeTabId,
    scale,
    gestureType,
    setCanvas,
    dragDelta,
    dragNodeIdsSet
  );

  const {
    variableDropMenu,
    setVariableDropMenu,
    handleNodeAddInput,
    handleNodeRemovePin,
    handleContextMenu,
  } = useCanvasDrop({
    canvasRef: ref,
    groupId,
    graphId: activeTabId,
    variables: { ...variables, ...Variables },
    functions,
    setNodes,
    setContextMenu,
    setPendingConnection,
    createNode: (nodeType: string, position: { x: number; y: number }, params?: Record<string, unknown>) => createNode(nodeType, position, params),
  });

  const activePin = useMemo(() => {
    if (gesturePinData) return gesturePinData;
    if (pendingConnection && contextMenu?.visible) return pendingConnection;
    return null;
  }, [gesturePinData, pendingConnection, contextMenu]);

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
      ref={ref}
      data-editor-group-id={groupId}
      className="relative w-full h-full overflow-hidden bg-[var(--workbench-bg)] select-none"
    >
      <ViewportGrid graphId={activeTabId ?? ""} />

      <div
        className="absolute inset-0"
        onPointerDown={onCanvasPointerDown}
        onContextMenu={handleContextMenu}
      >
        <ConnectionLine
          graphId={activeTabId ?? ""}
          getPinWorldPos={getPinWorldPos}
          getCanvasLocalPoint={getCanvasLocalPoint}
          pendingConnection={pendingConnection}
          menuPos={contextMenu}
        />

        <TransformContainer graphId={activeTabId ?? ""}>
          <EdgesOverlay
            graphId={activeTabId ?? ""}
            getPinWorldPos={getPinWorldPos}
            dimmed={isDraggingPin}
          />
          {graphNodeIds.map((nodeId: string) => {
            if (!visibleNodeIds.has(nodeId)) return null;
            const isSelected = selectedNodeIdsSet.has(nodeId);
            const isDragging = dragNodeIdsSet.has(nodeId);
            return (
              <CanvasNode
                key={nodeId}
                id={nodeId}
                graphId={activeTabId || undefined}
                groupId={groupId}
                scale={scale}
                selected={isSelected}
                dragDelta={isDragging ? (dragDelta ?? undefined) : undefined}
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

      <CanvasOverlays
        canvasRef={ref}
        variableDropMenu={variableDropMenu}
        setVariableDropMenu={setVariableDropMenu}
      />
    </div>
    </CanvasContextMenuProvider>
  );
}
