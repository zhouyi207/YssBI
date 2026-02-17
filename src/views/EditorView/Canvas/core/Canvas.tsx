import { useRef, useMemo } from "react";
import { Node } from "../../Nodes/Node";
import { useEditorGroup, useCanvasViewport, useCanvasDrop } from "@/features/application/editor";
import { useGestureStore } from "@/features/core/gesture";
import { useViewportStore } from "@/features/core/viewport";
import { useNodeManagement } from "@/features/application/dataManagement";
import { useExecutionVisualization } from "@/features/core/execution";

import { ViewportGrid } from "./ViewportGrid";
import { TransformContainer } from "./TransformContainer";
import { EdgesLayer } from "./EdgesLayer";
import { ConnectionLine } from "./ConnectionLine";
import CanvasOverlays from "../overlays/CanvasOverlays";

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
    saveHistory,
    activeTabId,
    pendingConnection,
    setPendingConnection,
    functions,
    macros,
    groupId,
    selectedNodeIds,
  } = useEditorGroup();

  const gesture = useGestureStore((state) => state.gesture);
  const { createNode } = useNodeManagement();

  const ref = useRef<HTMLDivElement>(null);
  const scale = useViewportStore((state) => state.viewports[groupId]?.scale || 1);

  const dragDelta = useMemo(() => {
    if (gesture?.type === "drag" && "dragDelta" in gesture && gesture.dragDelta)
      return gesture.dragDelta;
    return null;
  }, [gesture]);

  const { visibleNodeIds, getPinWorldPos, getCanvasLocalPoint } = useCanvasViewport(
    ref,
    groupId,
    activeTabId,
    nodes,
    scale,
    gesture,
    setCanvas,
    dragDelta,
    selectedNodeIds
  );

  const {
    variableDropMenu,
    setVariableDropMenu,
    handleNodeAddInput,
    handleContextMenu,
  } = useCanvasDrop({
    canvasRef: ref,
    groupId,
    variables: { ...variables, ...Variables },
    functions,
    macros,
    setNodes,
    setContextMenu,
    setPendingConnection,
    saveHistory,
    createNode: (nodeType: string, position: { x: number; y: number }) => createNode(nodeType, position),
  });

  const selectedNodeIdsSet = useMemo(
    () => new Set(selectedNodeIds),
    [selectedNodeIds]
  );

  const activePin = useMemo(() => {
    if (gesture?.type === "connect") return gesture.startPin;
    if (pendingConnection && contextMenu?.visible) return pendingConnection;
    return null;
  }, [gesture, pendingConnection, contextMenu]);

  const handlePinClick = () => {};

  return (
    <div
      ref={ref}
      className="relative w-full h-full overflow-hidden bg-[var(--workbench-bg)] select-none"
    >
      <ViewportGrid groupId={groupId} />

      <div
        className="absolute inset-0"
        onPointerDown={onCanvasPointerDown}
        onContextMenu={handleContextMenu}
      >
        <EdgesLayer
          groupId={groupId}
          getPinWorldPos={getPinWorldPos}
          activeTabId={activeTabId}
        />

        <ConnectionLine
          groupId={groupId}
          getPinWorldPos={getPinWorldPos}
          getCanvasLocalPoint={getCanvasLocalPoint}
          pendingConnection={pendingConnection}
          menuPos={contextMenu}
        />

        <TransformContainer groupId={groupId}>
          {nodes
            .filter((n: { id: string }) => visibleNodeIds.has(n.id))
            .map((node: { id: string }) => (
              <Node
                key={node.id}
                id={node.id}
                node={node as unknown as import('@/shared/types/ui').Node}
                scale={scale}
                selected={selectedNodeIdsSet.has(node.id)}
                dragDelta={dragDelta ?? undefined}
                activePinId={activePin?.id}
                subgraphId={activeTabId || undefined}
                onPointerDown={(id, e) => onNodePointerDown(id, e)}
                onAddInput={handleNodeAddInput}
                onPinClick={handlePinClick}
                onPinPointerDown={(e, p) => onPinPointerDown(p.id, e)}
                onPinValueChange={() => {}}
              />
            ))}
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
