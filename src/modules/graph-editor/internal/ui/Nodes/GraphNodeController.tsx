import { memo, useContext, useMemo, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { formatGraphDiagnostic } from "@/features/domain/graphDiagnostics/nodeDiagnostics";
import type { GraphContextMenuActions } from "@/features/application/editor";
import { useNodeView } from "@/features/core/dataStore/useNodeView";
import { isRerouteNodeView } from "@/features/core/dataStore/nodeView";
import { useGraphRead } from "@/features/core/graph/read";
import { graphElementState } from "@/features/application/results";
import { GraphFlowContext } from "../Canvas/core/GraphFlowContext";
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
  renderPinHandle?: (pin: PinData) => ReactNode;
  canConnectPin?: (pin: PinData) => boolean;
}

export const GraphNodeController = memo(function GraphNodeController({
  id,
  graphPath,
  groupId,
  selected,
  activePin,
  contextMenuActions,
  renderPinHandle,
  canConnectPin,
}: GraphNodeControllerProps) {
  const { i18n, t } = useTranslation();
  const node = useNodeView(id, graphPath);
  const presentation = useContext(GraphFlowContext)?.presentation;
  const cached = presentation?.nodes[id];
  const cacheState = cached?.cache ?? "new";
  const executionState = presentation
    ? graphElementState(
        presentation,
        id,
        cacheState,
        node?.diagnostics.some((diagnostic) => diagnostic.blocking),
      )
    : undefined;
  const isCompleted = cacheState === "valid";
  const hasError = executionState === "error";
  const graphBucket = useGraphRead((snapshot) =>
    graphPath ? snapshot.graphEntities[graphPath] : undefined,
  );
  const projectedNode = graphBucket?.nodes[id];
  const projectedCapabilities = projectedNode?.capabilities;
  const hasLinks =
    projectedNode?.pinIds.some((pinId) => (graphBucket?.pinConnections[pinId]?.length ?? 0) > 0) ??
    false;
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);

  const nodeDimmed = useMemo(() => {
    if (!node || !activePin || activePin.nodeId === node.id) return false;
    return ![...node.inputs, ...node.outputs].some((pin) =>
      canConnectPin ? canConnectPin(pin) : isPinCompatible(pin, activePin),
    );
  }, [activePin, canConnectPin, node]);

  if (!node) return null;

  const layoutProps = {
    node,
    activePinId: activePin?.id,
    activePin,
    graphPath,
    groupId,
    contextMenuActions,
    renderPinHandle,
    canConnectPin,
  };
  const isReroute = isRerouteNodeView(node);
  const contentSlot = isReroute ? (
    <RerouteNodeLayout
      node={node}
      activePinId={activePin?.id}
      graphPath={graphPath}
      groupId={groupId}
      contextMenuActions={contextMenuActions}
      renderPinHandle={renderPinHandle}
    />
  ) : (
    <DefaultNodeLayout {...layoutProps} />
  );
  const executionLabel = executionState ? t(`canvas.graphState.${executionState}`) : "";
  const failureLabel =
    presentation?.failure?.source?.nodeId === id
      ? t(`runFailure.causes.${presentation.failure.code}`, {
          defaultValue: t("runFailure.unknown"),
        })
      : "";
  const cacheLabel = cached
    ? `${t(`canvas.graphState.${cacheState}`)} · ${t("canvas.resultCount", { valid: cached.valid, total: cached.total })}`
    : "";
  const executionBadgeSlot = executionState ? (
    <div
      className="graph-state-badge"
      title={[executionLabel, failureLabel, cacheLabel].filter(Boolean).join(" · ")}
      aria-label={[executionLabel, failureLabel, cacheLabel].filter(Boolean).join(" · ")}
    >
      <span aria-hidden="true">
        {
          {
            unexecuted: "○",
            running: "▶",
            error: "!",
            valid: "✓",
            stale: "!",
            partial: "◐",
          }[executionState]
        }
      </span>
      {cached && (
        <span className="graph-cache-count" data-cache-state={cacheState}>
          {cached.valid}/{cached.total}
        </span>
      )}
    </div>
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
            aria-label={formatGraphDiagnostic(primaryDiagnostic, i18n?.resolvedLanguage)}
          />
        </TooltipTrigger>
        <TooltipContent side="top">
          {formatGraphDiagnostic(primaryDiagnostic, i18n?.resolvedLanguage)}
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
  const minSize = getNodeMinSize(isReroute);

  return (
    <GraphNodeView
      nodeId={node.id}
      executionState={executionState}
      cacheState={cacheState}
      className={className}
      style={{
        ...minSize,
        background: getNodeBackgroundStyle({ hasError, isCompleted }),
        opacity: nodeDimmed ? 0.35 : undefined,
        transition: "border-color 200ms, box-shadow 200ms, background 200ms, opacity 150ms",
        WebkitFontSmoothing: "antialiased",
        MozOsxFontSmoothing: "grayscale",
      }}
      contentSlot={contentSlot}
      executionBadgeSlot={executionBadgeSlot}
      diagnosticBadgeSlot={diagnosticBadgeSlot}
      contextMenuSlot={contextMenuSlot}
      onContextMenu={(event) => {
        event.preventDefault();
        event.stopPropagation();
        contextMenuActions?.selectNode(node.id, groupId);
        setContextMenu({ x: event.clientX, y: event.clientY });
      }}
    />
  );
});
