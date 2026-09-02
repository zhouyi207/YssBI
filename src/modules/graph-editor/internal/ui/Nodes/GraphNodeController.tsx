import { memo, useCallback, useMemo, useState } from "react";
import type { GraphContextMenuActions } from "@/features/application/editor";
import { useNodeView } from "@/features/core/dataStore/useNodeView";
import { isRerouteNodeView } from "@/features/core/dataStore/nodeView";
import { useExecutionRead } from "@/features/core/execution/read";
import { useGraphRead } from "@/features/core/graph/read";
import { useNodeExecution } from "@/features/core/node";
import {
  getNodeBackgroundStyle,
  getNodeClassName,
  getNodeMinSize,
} from "@/features/domain/node/utils";
import type { PinData } from "@/features/domain/editorProjection/graphRuntimeTypes";
import { isPinCompatible } from "@/features/domain/editorProjection/connectionRules";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { NodeContextMenu } from "../ContextMenu";
import { DefaultNodeLayout } from "./DefaultNodeLayout";
import { GraphNodeView } from "./GraphNodeView";
import { RerouteNodeLayout } from "./RerouteNodeLayout";

export interface GraphNodeControllerProps {
  id: string;
  graphPath?: string;
  groupId?: string;
  selected?: boolean;
  activePin?: PinData | null;
  contextMenuActions?: GraphContextMenuActions | null;
  onPointerDown?: (nodeId: string, event: React.PointerEvent) => void;
  onAddInput?: (id: string) => void;
  onRemovePin?: (nodeId: string, pinId: string) => void;
  onPinPointerDown?: (pin: PinData, event: React.PointerEvent) => void;
}

export const GraphNodeController = memo(function GraphNodeController({
  id,
  graphPath,
  groupId,
  selected,
  activePin,
  contextMenuActions,
  onPointerDown,
  onAddInput,
  onRemovePin,
  onPinPointerDown,
}: GraphNodeControllerProps) {
  const node = useNodeView(id, graphPath);
  const graphStatus = useExecutionRead((snapshot) =>
    graphPath ? (snapshot.graphs[graphPath]?.status ?? "idle") : "idle",
  );
  const isReplay = useExecutionRead((snapshot) =>
    Boolean(graphPath && snapshot.isPlaying && snapshot.playbackGraphPath === graphPath),
  );
  const useStoreExecVisual = graphStatus !== "running" && !isReplay;
  const { isCompleted, hasError } = useNodeExecution(id, graphPath, useStoreExecVisual);
  const graphBucket = useGraphRead((snapshot) =>
    graphPath ? snapshot.graphEntities[graphPath] : undefined,
  );
  const projectedCapabilities = graphBucket?.nodes[id]?.capabilities;
  const hasLinks =
    graphBucket?.nodePins[id]?.some(
      (pinId) => (graphBucket.pinConnections[pinId]?.length ?? 0) > 0,
    ) ?? false;
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);

  const nodeDimmed = useMemo(() => {
    if (!node || !activePin || activePin.nodeId === node.id) return false;
    return ![...node.inputs, ...node.outputs].some((pin) => isPinCompatible(pin, activePin));
  }, [activePin, node]);
  const handlePinPointerDown = useCallback(
    (event: React.PointerEvent, pin: PinData) => {
      onPinPointerDown?.(pin, event);
    },
    [onPinPointerDown],
  );

  if (!node) return null;

  const layoutProps = {
    node,
    activePinId: activePin?.id,
    activePin,
    graphPath,
    groupId,
    contextMenuActions,
    onAddInput,
    onRemovePin,
    onPinPointerDown: handlePinPointerDown,
  };
  const isReroute = isRerouteNodeView(node);
  const contentSlot = isReroute ? (
    <RerouteNodeLayout
      node={node}
      activePinId={activePin?.id}
      graphPath={graphPath}
      groupId={groupId}
      contextMenuActions={contextMenuActions}
      onPinPointerDown={handlePinPointerDown}
    />
  ) : (
    <DefaultNodeLayout {...layoutProps} />
  );
  const executionBadgeSlot = hasError ? (
    <div className="absolute -top-1 -right-1 h-3 w-3 rounded-full bg-red-500" />
  ) : isCompleted ? (
    <div className="absolute -top-1.5 -right-1.5 h-4 w-4 rounded-full bg-green-500 shadow-lg shadow-green-500/40" />
  ) : null;
  const primaryDiagnostic =
    node.diagnostics.find((diagnostic) => diagnostic.blocking) ?? node.diagnostics[0];
  const diagnosticBadgeSlot =
    !hasError && primaryDiagnostic ? (
      <Tooltip>
        <TooltipTrigger asChild>
          <div
            className={`absolute -top-1 -left-1 h-3 w-3 rounded-full shadow-sm ${
              primaryDiagnostic.severity === "error"
                ? "bg-red-500"
                : primaryDiagnostic.severity === "warning"
                  ? "bg-amber-400"
                  : "bg-blue-400"
            }`}
            aria-label={primaryDiagnostic.message}
          />
        </TooltipTrigger>
        <TooltipContent side="top">{primaryDiagnostic.message}</TooltipContent>
      </Tooltip>
    ) : null;
  const contextMenuSlot =
    contextMenu && contextMenuActions ? (
      <NodeContextMenu
        position={contextMenu}
        capabilities={projectedCapabilities}
        hasLinks={hasLinks}
        onCopy={() => contextMenuActions.copyNode(node.id)}
        onCut={() => void contextMenuActions.cutNode(node.id)}
        onDuplicate={() => void contextMenuActions.duplicateNode(node.id)}
        onDelete={() => void contextMenuActions.deleteNode(node.id)}
        onBreakAllLinks={() => void contextMenuActions.breakAllNodeLinks(node.id)}
        onSelectLinked={() => contextMenuActions.selectLinkedNodes(node.id)}
        onClose={() => setContextMenu(null)}
      />
    ) : null;
  const className = getNodeClassName({
    selected: selected || contextMenu != null,
    hasError,
    isCompleted,
  });
  const minSize = getNodeMinSize(isReroute);

  return (
    <GraphNodeView
      nodeId={node.id}
      className={className}
      style={{
        ...minSize,
        transform: `translate3d(${node.position.x}px, ${node.position.y}px, 0)`,
        background: getNodeBackgroundStyle({ hasError, isCompleted }),
        opacity: nodeDimmed ? 0.35 : undefined,
        transition: useStoreExecVisual
          ? "border-color 200ms, box-shadow 200ms, background 200ms, opacity 150ms"
          : undefined,
        WebkitFontSmoothing: "antialiased",
        MozOsxFontSmoothing: "grayscale",
      }}
      contentSlot={contentSlot}
      executionBadgeSlot={executionBadgeSlot}
      diagnosticBadgeSlot={diagnosticBadgeSlot}
      contextMenuSlot={contextMenuSlot}
      onPointerDown={(event) => onPointerDown?.(node.id, event)}
      onContextMenu={(event) => {
        event.preventDefault();
        event.stopPropagation();
        contextMenuActions?.selectNode(node.id, groupId);
        setContextMenu({ x: event.clientX, y: event.clientY });
      }}
    />
  );
});
