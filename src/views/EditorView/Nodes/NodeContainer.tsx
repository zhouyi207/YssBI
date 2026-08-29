import React, { useState, useCallback } from "react";
import type { UINode } from "@/shared/types/ui";
import { uiNodeHasNoHeader, uiNodeIsReroute } from "@/features/core/dataStore/nodeView";
import { useNodeExecution } from "@/features/core/node";
import { useExecutionRead } from '@/features/core/execution/read';
import { useGraphRead } from '@/features/core/graph/read';
import { getNodeClassName, getNodeBackgroundStyle, getNodeMinSize } from "@/features/domain/node/utils";
import { useCanvasContextMenuActionsOptional } from "@/features/application/editor/CanvasContextMenuContext";
import { useCallFunctionIssue } from "@/features/application/graphDiagnostics/useCallFunctionDiagnostics";
import { NodeContextMenu } from "../ContextMenu";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useTranslation } from "react-i18next";

interface NodeContainerProps {
  node: UINode;
  graphPath?: string;
  groupId?: string;
  selected?: boolean;
  dimmed?: boolean;
  onPointerDown?: (nodeId: string, e: React.PointerEvent) => void;
  children: React.ReactNode;
}

export const NodeContainer = React.memo<NodeContainerProps>(({
  node,
  graphPath: _graphPath,
  groupId,
  selected,
  dimmed,
  onPointerDown,
  children,
}) => {
  const { t } = useTranslation();
  const posX = node.position.x;
  const posY = node.position.y;
  const graphStatus = useExecutionRead((snapshot) =>
    _graphPath ? snapshot.graphs[_graphPath]?.status ?? 'idle' : 'idle',
  );
  const isReplay = useExecutionRead((snapshot) =>
    Boolean(_graphPath && snapshot.isPlaying && snapshot.playbackGraphPath === _graphPath),
  );
  const useStoreExecVisual = graphStatus !== 'running' && !isReplay;

  const { isCompleted, hasError } = useNodeExecution(node.id, _graphPath, useStoreExecVisual);
  const callIssue = useCallFunctionIssue(_graphPath, node.id);
  const menuActions = useCanvasContextMenuActionsOptional();

  const graphBucket = useGraphRead((snapshot) =>
    _graphPath ? snapshot.graphEntities[_graphPath] : undefined,
  );
  const projectedCapabilities = graphBucket?.nodes[node.id]?.capabilities;
  const hasLinks = graphBucket?.nodePins[node.id]
    ?.some((pinId) => (graphBucket.pinConnections[pinId]?.length ?? 0) > 0) ?? false;

  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);

  const handleContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    menuActions?.selectNode(node.id, groupId);
    setContextMenu({ x: e.clientX, y: e.clientY });
  }, [menuActions, node.id, groupId]);

  const className = getNodeClassName({
    selected: selected || !!contextMenu,
    hasError,
    isCompleted,
  });

  const background = getNodeBackgroundStyle({
    hasError,
    isCompleted,
  });

  const minSize = getNodeMinSize(uiNodeHasNoHeader(node), uiNodeIsReroute(node));

  return (
    <div
      id={node.id}
      data-node-id={node.id}
      className={className}
      style={{
        ...minSize,
        transform: `translate3d(${posX}px, ${posY}px, 0)`,
        background,
        opacity: dimmed ? 0.35 : undefined,
        transition: useStoreExecVisual ? "border-color 200ms, box-shadow 200ms, background 200ms, opacity 150ms" : undefined,
        WebkitFontSmoothing: "antialiased",
        MozOsxFontSmoothing: "grayscale",
      }}
      onPointerDown={(e) => onPointerDown?.(node.id, e)}
      onContextMenu={handleContextMenu}
    >
      {children}

      {hasError && (
        <div className="absolute -top-1 -right-1 w-3 h-3 bg-red-500 rounded-full" />
      )}
      {!hasError && callIssue && (
        <Tooltip>
          <TooltipTrigger asChild>
            <div
              className="absolute -top-1 -left-1 h-3 w-3 rounded-full bg-amber-400 shadow-sm"
              aria-label={t('graphDiagnostics.callFunctionNodeBadge')}
            />
          </TooltipTrigger>
          <TooltipContent side="top">
            {callIssue.kind === 'empty_target'
              ? t('graphDiagnostics.callFunctionEmptyTarget')
              : t('graphDiagnostics.callFunctionMissingTarget', { path: callIssue.subGraphPath ?? '' })}
          </TooltipContent>
        </Tooltip>
      )}
      {isCompleted && (
        <div className="absolute -top-1.5 -right-1.5 w-4 h-4 bg-green-500 rounded-full shadow-lg shadow-green-500/40" />
      )}

      {contextMenu && menuActions && (
        <NodeContextMenu
          position={contextMenu}
          capabilities={projectedCapabilities}
          hasLinks={hasLinks}
          onCopy={() => menuActions.copyNode(node.id)}
          onCut={() => void menuActions.cutNode(node.id)}
          onDuplicate={() => void menuActions.duplicateNode(node.id)}
          onDelete={() => void menuActions.deleteNode(node.id)}
          onBreakAllLinks={() => void menuActions.breakAllNodeLinks(node.id)}
          onSelectLinked={() => menuActions.selectLinkedNodes(node.id)}
          onClose={() => setContextMenu(null)}
        />
      )}
    </div>
  );
});
