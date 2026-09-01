import { memo, useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type { GraphContextMenuActions } from "@/features/application/editor";
import { useCallFunctionIssue } from "@/features/application/graphDiagnostics/useCallFunctionDiagnostics";
import { useNodeView } from "@/features/core/dataStore/useNodeView";
import { uiNodeHasNoHeader, uiNodeIsReroute } from "@/features/core/dataStore/nodeView";
import { useExecutionRead } from "@/features/core/execution/read";
import { useGraphRead } from "@/features/core/graph/read";
import { useNodeExecution } from "@/features/core/node";
import {
  getNodeBackgroundStyle,
  getNodeClassName,
  getNodeMinSize,
} from "@/features/domain/node/utils";
import type { Pin as PinModel } from "@/shared/types/domain";
import { isPinCompatible } from "@/features/domain/editorProjection/connectionRules";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { NodeContextMenu } from "../ContextMenu";
import { DefaultNodeLayout } from "./DefaultNodeLayout";
import { GraphNodeView } from "./GraphNodeView";
import { MathNodeLayout } from "./MathNodeLayout";
import { RerouteNodeLayout } from "./RerouteNodeLayout";

export interface GraphNodeControllerProps {
  id: string;
  graphPath?: string;
  groupId?: string;
  selected?: boolean;
  activePin?: PinModel | null;
  contextMenuActions?: GraphContextMenuActions | null;
  onPointerDown?: (nodeId: string, event: React.PointerEvent) => void;
  onAddInput?: (id: string) => void;
  onRemovePin?: (nodeId: string, pinId: string) => void;
  onPinClick?: (pinId: string, direction: "input" | "output") => void;
  onPinPointerDown?: (pin: PinModel, event: React.PointerEvent) => void;
  onPinValueChange?: (pinId: string, value: unknown) => void;
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
  onPinClick,
  onPinPointerDown,
  onPinValueChange,
}: GraphNodeControllerProps) {
  const { t } = useTranslation();
  const node = useNodeView(id, graphPath);
  const graphStatus = useExecutionRead((snapshot) =>
    graphPath ? (snapshot.graphs[graphPath]?.status ?? "idle") : "idle",
  );
  const isReplay = useExecutionRead((snapshot) =>
    Boolean(graphPath && snapshot.isPlaying && snapshot.playbackGraphPath === graphPath),
  );
  const useStoreExecVisual = graphStatus !== "running" && !isReplay;
  const { isCompleted, hasError } = useNodeExecution(id, graphPath, useStoreExecVisual);
  const callIssue = useCallFunctionIssue(graphPath, id);
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
    (event: React.PointerEvent, pin: PinModel) => {
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
    onPinClick,
    onPinPointerDown: handlePinPointerDown,
    onPinValueChange,
  };
  const contentSlot = uiNodeIsReroute(node) ? (
    <RerouteNodeLayout
      node={node}
      activePinId={activePin?.id}
      graphPath={graphPath}
      groupId={groupId}
      contextMenuActions={contextMenuActions}
      onPinClick={onPinClick}
      onPinPointerDown={handlePinPointerDown}
    />
  ) : node.uiStyle === "math" ? (
    <MathNodeLayout {...layoutProps} />
  ) : (
    <DefaultNodeLayout {...layoutProps} />
  );
  const executionBadgeSlot = hasError ? (
    <div className="absolute -top-1 -right-1 h-3 w-3 rounded-full bg-red-500" />
  ) : isCompleted ? (
    <div className="absolute -top-1.5 -right-1.5 h-4 w-4 rounded-full bg-green-500 shadow-lg shadow-green-500/40" />
  ) : null;
  const diagnosticBadgeSlot =
    !hasError && callIssue ? (
      <Tooltip>
        <TooltipTrigger asChild>
          <div
            className="absolute -top-1 -left-1 h-3 w-3 rounded-full bg-amber-400 shadow-sm"
            aria-label={t("graphDiagnostics.callFunctionNodeBadge")}
          />
        </TooltipTrigger>
        <TooltipContent side="top">
          {callIssue.kind === "empty_target"
            ? t("graphDiagnostics.callFunctionEmptyTarget")
            : t("graphDiagnostics.callFunctionMissingTarget", {
                path: callIssue.subGraphPath ?? "",
              })}
        </TooltipContent>
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
  const minSize = getNodeMinSize(uiNodeHasNoHeader(node), uiNodeIsReroute(node));

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
