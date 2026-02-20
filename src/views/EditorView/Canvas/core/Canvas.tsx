import { useRef, useMemo, useCallback } from "react";
import { Node } from "../../Nodes/Node";
import { useEditorGroup, useCanvasViewport, useCanvasDrop } from "@/features/application/editor";
import { useGestureStore } from "@/features/core/gesture";
import { useViewportStore } from "@/features/core/viewport";
import { useNodeManagement } from "@/features/application/dataManagement";
import { useExecutionVisualization } from "@/features/core/execution";

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

export default function Canvas() {
  useExecutionVisualization();

  const {
    nodes,
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
    macros,
    groupId,
    selectedNodeIds,
  } = useEditorGroup();

  const dragDelta = useGestureStore(selectDragDelta, dragDeltaEq);
  const dragNodeIds = useGestureStore(selectDragNodeIds);
  const gestureType = useGestureStore(selectGestureType);
  const gesturePinData = useGestureStore(selectActivePin);

  const { createNode } = useNodeManagement();

  const ref = useRef<HTMLDivElement>(null);
  const scale = useViewportStore((state) => state.viewports[groupId]?.scale || 1);

  const selectedNodeIdsSet = useMemo(
    () => new Set(selectedNodeIds),
    [selectedNodeIds]
  );

  const dragNodeIdsSet = useMemo(
    () => (dragNodeIds ? new Set(dragNodeIds) : new Set<string>()),
    [dragNodeIds]
  );

  const { visibleNodeIds, getPinWorldPos, getCanvasLocalPoint } = useCanvasViewport(
    ref,
    groupId,
    activeTabId,
    nodes,
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
    handleContextMenu,
  } = useCanvasDrop({
    canvasRef: ref,
    groupId,
    graphId: activeTabId,
    variables: { ...variables, ...Variables },
    functions,
    macros,
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

  const handlePinClick = useCallback(() => {}, []);
  const handlePinValueChange = useCallback(() => {}, []);

  return (
    <div
      ref={ref}
      data-editor-group-id={groupId}
      className="relative w-full h-full overflow-hidden bg-[var(--workbench-bg)] select-none"
    >
      <ViewportGrid groupId={groupId} />

      <div
        className="absolute inset-0"
        onPointerDown={onCanvasPointerDown}
        onContextMenu={handleContextMenu}
      >
        <ConnectionLine
          groupId={groupId}
          getPinWorldPos={getPinWorldPos}
          getCanvasLocalPoint={getCanvasLocalPoint}
          pendingConnection={pendingConnection}
          menuPos={contextMenu}
        />

        <TransformContainer groupId={groupId}>
          <EdgesOverlay
            nodes={nodes}
            getPinWorldPos={getPinWorldPos}
          />
          {nodes
            .filter((n: { id: string }) => visibleNodeIds.has(n.id))
            .map((node: { id: string }) => {
              const isSelected = selectedNodeIdsSet.has(node.id);
              const isDragging = dragNodeIdsSet.has(node.id);
              return (
                <Node
                  key={node.id}
                  id={node.id}
                  node={node as unknown as import('@/shared/types/ui').Node}
                  scale={scale}
                  selected={isSelected}
                  dragDelta={isDragging ? (dragDelta ?? undefined) : undefined}
                  activePinId={activePin?.id}
                  subgraphId={activeTabId || undefined}
                  onPointerDown={onNodePointerDown}
                  onAddInput={handleNodeAddInput}
                  onPinClick={handlePinClick}
                  onPinPointerDown={(e, p) => onPinPointerDown(p.id, e)}
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
  );
}
