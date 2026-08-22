import React, { useMemo, useState, useCallback } from "react";
import { useTranslation } from "react-i18next";
import type { PinMetaDataDTO } from "@/shared/types/domain";
import { Pin as PinModel } from "@/shared/types/domain";
import { useTheme } from "@/features/core/theme/useTheme";
import { getPinTypeColor } from "@/features/core/theme/pinTypeTheme";
import { isExecPin, scalarPinInputKey, PRIMITIVE_SCALAR_INPUT_KEYS } from "@/shared/types/domain/pinSemantics";
import { resolvePinRenderStyle, resolvePinVisualSpec } from "@/shared/types/domain/pinVisual";
import { PinInput } from "./PinInput";
import { PinContextMenu } from "../ContextMenu";
import { useCanvasContextMenuActionsOptional } from "@/features/application/editor/CanvasContextMenuContext";
import { useRepeatablePinRemovable } from "@/features/core/pin";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { dataValueFromBackend } from "@/shared/types/dto/dataValue";
import { dataValueToRaw } from "@/shared/types/domain/dataValue";
import { useGraphDataStore } from "@/features/core/dataStore";
import { getCanvasInteraction, useGraphInteractionStore } from '@/features/core/graphInteraction/graphInteractionStore';
import type { ConnectionFeedback } from '@/features/core/canvas/connectionInteraction';

export function pinConnectionFeedbackAttributes(feedback: ConnectionFeedback | null) {
  if (!feedback) return {};
  return feedback.kind === 'invalid'
    ? {
        'data-connection-feedback': feedback.kind,
        'data-connection-invalid-reason': feedback.reason,
      }
    : { 'data-connection-feedback': feedback.kind };
}

export function pinConnectionFeedbackClass(feedback: ConnectionFeedback | null): string {
  if (!feedback) return '';
  if (feedback.kind === 'invalid') return 'ring-2 ring-red-500/90';
  return feedback.kind === 'replace'
    ? 'ring-2 ring-amber-500/90'
    : 'ring-2 ring-emerald-500/90';
}
import {
  buildPinViewParams,
  evaluatePinViewState,
  pinHistoryCacheKey,
  pinViewDisabledTitle,
  useExecutionStore,
} from "@/features/core/execution";
import { openPinInspectableView } from "@/features/application/execution/openInspectableResult";
import {
  isPinPreviewActionAvailable,
  requestAndOpenPinPreview,
} from "@/features/application/editor/requestPinPreview";
import type { PinHistoryProjection } from '@/shared/types/ui';
import type {
  PortAddressDto,
  PortKindDto,
  ResolvedPortStatusDto,
} from "@/shared/types/dto/editorProjection";

/** 将 userValue 转为可显示/编辑的原始值（兼容 DataValue DTO 与本地 raw 格式） */
function toDisplayValue(v: unknown): unknown {
  if (v == null) return v;
  if (typeof v === "object" && !Array.isArray(v) && ("String" in v || "Boolean" in v || "Int64" in v || "Float64" in v || "Null" in v)) {
    return dataValueToRaw(dataValueFromBackend(v as Parameters<typeof dataValueFromBackend>[0]));
  }
  return v;
}

export type PinDragState = "normal" | "highlighted" | "dimmed";

const EMPTY_CONNECTION_IDS: string[] = [];

export interface PinProps extends PinModel {
  connected?: boolean;
  linkCount?: number;
  connectionIds?: string[];
  /** 来自 schema 的 pin metaData（如 dropdown 的 widgetOptions） */
  metaData?: PinMetaDataDTO;
  graphPath?: string;
  groupId?: string;
  onPinClick?: (id: string, direction: "input" | "output") => void;
  onPinPointerDown?: (e: React.PointerEvent, pin: PinModel) => void;
  isActive?: boolean;
  pinDragState?: PinDragState;
  onValueChange?: (pinId: string, value: unknown) => void;
  onRemovePin?: (pinId: string) => void;
  forceShowInput?: boolean;
  /** Rust-projected preview eligibility fields. */
  address?: PortAddressDto;
  kind?: PortKindDto;
  orphan?: boolean;
  status?: ResolvedPortStatusDto;
}

export const Pin: React.FC<PinProps> = (props) => {
  const {
    id,
    nodeId,
    name,
    type,
    direction,
    connected = false,
    linkCount = 0,
    ui,
    metaData,
    graphPath,
    groupId,
    onPinClick,
    onPinPointerDown,
    isActive,
    pinDragState = "normal",
    dataType,
    optional,
    defaultValue,
    userValue,
    onValueChange,
    onRemovePin,
    forceShowInput,
    validationWarning,
    address,
    kind,
    orphan,
    status,
  } = props;

  const { t } = useTranslation();
  const { theme: appTheme } = useTheme();
  const isConnected = connected || linkCount > 0 || (isActive ?? false);
  const pinSemantics = useMemo(
    () => ({ type: type ?? 'object', dataType }),
    [type, dataType],
  );
  const visualSpec = useMemo(() => resolvePinVisualSpec(pinSemantics), [pinSemantics]);
  const baseColor = ui?.color ?? getPinTypeColor(visualSpec.colorKey, appTheme);

  const renderStyle = useMemo(
    () => resolvePinRenderStyle(visualSpec, isConnected, baseColor),
    [visualSpec, isConnected, baseColor],
  );

  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);
  const [historyProjection, setHistoryProjection] = useState<PinHistoryProjection>();
  const menuActions = useCanvasContextMenuActionsOptional();
  const canRemoveRepeatable = useRepeatablePinRemovable(nodeId, id, graphPath);
  const canRemovePin =
    canRemoveRepeatable && (onRemovePin != null || menuActions?.removeRepeatablePin != null);

  const connectionFeedback = useGraphInteractionStore((state) => {
    if (!graphPath || !groupId) return null;
    const interaction = getCanvasInteraction(state, graphPath, groupId);
    if (interaction?.type !== 'drawingConnection' && interaction?.type !== 'movingConnections') return null;
    const session = interaction.session;
    return session.snappedTarget?.id === id || session.hoveredTarget?.id === id
      ? session.feedback
      : null;
  });
  const connectionIds = useGraphDataStore((state) =>
    graphPath ? state.getGraphPinConnections(graphPath, id) : EMPTY_CONNECTION_IDS,
  );
  const graphConnections = useGraphDataStore((state) =>
    graphPath ? state.graphEntities[graphPath]?.connections : undefined,
  );
  const connections = useMemo(
    () => connectionIds.flatMap((connectionId) => {
      const connection = graphConnections?.[connectionId];
      return connection?.output && connection.input
        ? [{
            connectionId: connection.id,
            output: connection.output,
            input: connection.input,
            order: connection.order ?? null,
          }]
        : [];
    }),
    [connectionIds, graphConnections],
  );

  const viewParams = useMemo(
    () =>
      graphPath
        ? buildPinViewParams({
            graphPath: graphPath,
            address,
            direction,
            isExec: isExecPin(pinSemantics),
            connections,
          })
        : null,
    [graphPath, address, direction, pinSemantics, connections],
  );

  const viewState = useMemo(
    () =>
      viewParams
        ? evaluatePinViewState(viewParams)
        : null,
    [viewParams],
  );

  const previewActionAvailable = isPinPreviewActionAvailable(graphPath, {
    direction,
    kind,
    address,
    orphan,
    status,
  });
  const showViewMenu = (viewState?.showMenu ?? false) || previewActionAvailable;
  const viewEnabled = (viewState?.enabled ?? false) || previewActionAvailable;
  const viewDisabledReason = previewActionAvailable
    ? null
    : (viewState?.disabledReason ?? null);
  const historyOutputs = useMemo(() => viewState?.refs.flatMap((ref) =>
    ref.kind === 'outputPin' ? [ref.output] : [],
  ) ?? [], [viewState]);
  const firstHistoryOutput = historyOutputs[0];

  const handleRemovePin = useCallback(() => {
    if (onRemovePin) {
      onRemovePin(id);
      return;
    }
    void menuActions?.removeRepeatablePin(nodeId, id);
  }, [onRemovePin, menuActions, nodeId, id]);

  const handleView = useCallback(() => {
    if (!viewParams || !graphPath) return;
    if (viewState?.enabled) {
      void openPinInspectableView(viewParams, t).then(() => {
        if (!firstHistoryOutput) return;
        setHistoryProjection(useExecutionStore.getState().graphs[graphPath]?.pinHistories.get(
          pinHistoryCacheKey(graphPath, firstHistoryOutput),
        ));
      });
      return;
    }
    if (previewActionAvailable) {
      void requestAndOpenPinPreview(graphPath, id, t);
    }
  }, [firstHistoryOutput, graphPath, id, previewActionAvailable, t, viewParams, viewState?.enabled]);

  const hasLinks = linkCount > 0 || (connectionIds?.length ?? 0) > 0;
  const scalarInputKey = scalarPinInputKey(dataType);
  const canReset =
    direction === "input" &&
    scalarInputKey != null &&
    PRIMITIVE_SCALAR_INPUT_KEYS.has(scalarInputKey) &&
    !visualSpec.container &&
    userValue != null &&
    userValue !== undefined;

  const handleContextMenu = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      e.stopPropagation();
      setContextMenu({ x: e.clientX, y: e.clientY });
    },
    []
  );

  const shouldPulse = !optional && !isConnected && direction === "input" && !isExecPin(pinSemantics);

  const isDropdownPin = metaData?.showWidget && metaData?.widgetType === "dropdown" && (metaData?.widgetOptions?.length ?? 0) > 0;
  const showInput =
    (!isConnected || forceShowInput === true) &&
    scalarInputKey != null &&
    (PRIMITIVE_SCALAR_INPUT_KEYS.has(scalarInputKey) || (scalarInputKey === "string" && isDropdownPin)) &&
    !visualSpec.container &&
    (direction === "input" || forceShowInput === true) &&
    graphPath &&
    nodeId;

  const effectivePinDragState = contextMenu ? "highlighted" : pinDragState;
  const dragStyle: React.CSSProperties | undefined =
    effectivePinDragState === "dimmed"
      ? { opacity: 0.25, transition: "opacity 150ms, filter 150ms" }
      : effectivePinDragState === "highlighted"
        ? { filter: "brightness(1.25) saturate(1.4)", transition: "opacity 150ms, filter 150ms" }
        : undefined;

  const feedbackTooltip = connectionFeedback?.kind === 'invalid'
    ? t(`canvas.connection.feedback.${connectionFeedback.reason}`)
    : null;
  const pinTooltip = feedbackTooltip ?? (validationWarning
    ? `${name} (${visualSpec.label}) — ${validationWarning}`
    : `${name} (${visualSpec.label})`);

  const pulseStrokeProps = shouldPulse
    ? {
        fill: 'none' as const,
        stroke: baseColor,
        strokeWidth: 2.5,
        strokeDasharray: visualSpec.shape === 'exec' ? '6 24' : visualSpec.shape === 'gridRect' ? '8 28' : '7 21',
        className: 'pin-flow-stroke',
        filter: 'url(#pinGlow)',
      }
    : null;

  const renderPinShape = () => {
    const { fill, stroke, strokeWidth } = renderStyle;
    const dashed = visualSpec.dashedStroke && !shouldPulse ? { strokeDasharray: '2 2' } : {};

    switch (visualSpec.shape) {
      case 'exec':
        return (
          <>
            <path
              d="M2 2 L7 2 L11 6 L7 10 L2 10 Z"
              fill={fill}
              stroke={stroke}
              strokeWidth={strokeWidth}
              strokeLinejoin="miter"
              {...dashed}
            />
            {pulseStrokeProps && <path d="M2 2 L7 2 L11 6 L7 10 L2 10 Z" strokeLinejoin="miter" {...pulseStrokeProps} />}
          </>
        );
      case 'gridRect':
        return (
          <>
            <g>
              <rect x="1.5" y="1.5" width="9" height="9" rx="1" fill={fill} stroke={stroke} strokeWidth={strokeWidth} {...dashed} />
              <line x1="1.5" y1="4.5" x2="10.5" y2="4.5" stroke={stroke} strokeWidth="0.8" />
              <line x1="5" y1="1.5" x2="5" y2="10.5" stroke={stroke} strokeWidth="0.8" />
            </g>
            {pulseStrokeProps && <rect x="1.5" y="1.5" width="9" height="9" rx="1" {...pulseStrokeProps} />}
          </>
        );
      case 'roundedRect':
        return (
          <>
            <rect x="2" y="2" width="8" height="8" rx="1.5" fill={fill} stroke={stroke} strokeWidth={strokeWidth} {...dashed} />
            {pulseStrokeProps && <rect x="2" y="2" width="8" height="8" rx="1.5" {...pulseStrokeProps} />}
          </>
        );
      case 'diamond':
        return (
          <>
            <polygon points="6,1 11,6 6,11 1,6" fill={fill} stroke={stroke} strokeWidth={strokeWidth} strokeLinejoin="miter" {...dashed} />
            {pulseStrokeProps && <polygon points="6,1 11,6 6,11 1,6" strokeLinejoin="miter" {...pulseStrokeProps} />}
          </>
        );
      case 'hexagon':
        return (
          <>
            <polygon
              points="6,0.5 10.8,3.25 10.8,8.75 6,11.5 1.2,8.75 1.2,3.25"
              fill={fill}
              stroke={stroke}
              strokeWidth={strokeWidth}
              strokeLinejoin="round"
              {...dashed}
            />
            {pulseStrokeProps && (
              <polygon
                points="6,0.5 10.8,3.25 10.8,8.75 6,11.5 1.2,8.75 1.2,3.25"
                strokeLinejoin="round"
                {...pulseStrokeProps}
              />
            )}
          </>
        );
      default:
        return (
          <>
            <circle cx="6" cy="6" r="4.5" fill={fill} stroke={stroke} strokeWidth={strokeWidth} {...dashed} />
            {pulseStrokeProps && <circle cx="6" cy="6" r="4.5" {...pulseStrokeProps} />}
          </>
        );
    }
  };

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <div
          className={`
       group relative flex items-center h-7 shrink-0 pin-container transition-opacity
        ${direction === "input"
          ? "flex-row justify-start"
          : "flex-row-reverse justify-end"
        }
      `}
          style={dragStyle}
          data-pin-id={id}
          {...pinConnectionFeedbackAttributes(connectionFeedback)}
          data-validation-warning={validationWarning ? 'true' : undefined}
          onContextMenu={handleContextMenu}
        >
      {/* Pin Icon Container - 唯一连接锚点 */}
      <div
        data-pin-connection-anchor={id}
        className={`
          relative w-6 h-6 flex items-center justify-center cursor-crosshair shrink-0 z-20 pin-circle rounded-full
          ${direction === "input" ? "mr-1" : "ml-1"}
          ${contextMenu ? "ring-2 ring-[var(--accent-color)]/60" : ""}
          ${validationWarning ? "ring-2 ring-amber-500/80" : ""}
          ${pinConnectionFeedbackClass(connectionFeedback)}
        `}
        onPointerDown={(e) => {
          if (!onPinPointerDown) return;
          e.stopPropagation();
          e.preventDefault();
          onPinPointerDown(e, props);
        }}
        onClick={(e) => {
          e.stopPropagation();
          onPinClick?.(id, direction);
        }}
      >
        <svg
          width="12"
          height="12"
          viewBox="0 0 12 12"
          className="overflow-visible"
          style={{ display: "block" }}
        >
          {renderPinShape()}
          {shouldPulse && (
            <defs>
              <filter id="pinGlow" x="-50%" y="-50%" width="200%" height="200%">
                <feGaussianBlur in="SourceGraphic" stdDeviation="1.5" />
              </filter>
            </defs>
          )}
          {isConnected && visualSpec.edgeKind === 'data' && (
            <circle
              cx="6"
              cy="6"
              r="1.2"
              fill="white"
              className="pointer-events-none"
            />
          )}
        </svg>
      </div>

      {/* Label - 增加 hover 效果，右键菜单打开时高亮 */}
      <span
        className={`
          text-[10px] font-bold select-none tracking-wide px-1 z-10 pointer-events-none
          transition-colors
          ${contextMenu
            ? "text-[var(--accent-color)]"
            : isConnected
              ? "text-foreground"
              : "text-muted-foreground"}
          ${!contextMenu ? "group-hover:text-foreground" : ""}
        `}
      >
        {name}
      </span>

      {showInput && (
        <PinInput
          pinId={id}
          nodeId={nodeId}
          graphPath={graphPath}
          dataType={dataType}
          metaData={metaData}
          value={toDisplayValue(userValue ?? defaultValue)}
          onValueChange={(value) => onValueChange?.(id, value)}
        />
      )}

      {contextMenu && (
        <PinContextMenu
          position={contextMenu}
          removable={canRemovePin}
          hasLinks={hasLinks}
          canReset={canReset}
          onBreakLinks={menuActions ? () => void menuActions.disconnectPin(id) : undefined}
          onResetValue={menuActions ? () => void menuActions.resetPinValue(nodeId, id) : undefined}
          showView={showViewMenu}
          viewEnabled={viewEnabled}
          viewDisabledTitle={pinViewDisabledTitle(viewDisabledReason, t)}
          onView={handleView}
          historyEntries={historyProjection?.entries}
          onViewHistory={(resultId) => {
            if (!historyProjection) return;
            useExecutionStore.getState().recordPinHistory({
              ...historyProjection,
              selectedResultId: resultId,
            });
            void openPinInspectableView(viewParams!, t, { selectedResultId: resultId });
          }}
          onRemove={handleRemovePin}
          onClose={() => setContextMenu(null)}
        />
      )}
        </div>
      </TooltipTrigger>
      <TooltipContent side={direction === 'input' ? 'left' : 'right'}>{pinTooltip}</TooltipContent>
    </Tooltip>
  );
};
