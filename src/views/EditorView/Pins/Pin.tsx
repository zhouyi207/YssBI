import React, { useMemo, useState, useCallback } from "react";
import { useTranslation } from "react-i18next";
import type { PinMetaDataDTO } from "@/shared/types/domain";
import { Pin as PinModel } from "@/shared/types/domain";
import { useTheme } from "@/features/core/theme/useTheme";
import { getPinTypeColor } from "@/features/core/theme/pinTypeTheme";
import { pinThemeTypeKey, isExecPin, pinTypeLabel, scalarPinInputKey, PRIMITIVE_SCALAR_INPUT_KEYS } from "@/shared/types/domain/pinSemantics";
import { PinInput } from "./PinInput";
import { PinContextMenu } from "../ContextMenu";
import { useCanvasContextMenuActionsOptional } from "@/features/application/editor/CanvasContextMenuContext";
import { useRepeatablePinRemovable } from "@/features/core/pin";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { dataValueFromBackend } from "@/shared/types/dto/dataValue";
import { dataValueToRaw } from "@/shared/types/domain/dataValue";
import { useGraphDataStore } from "@/features/core/dataStore";
import {
  buildPinViewParams,
  evaluatePinViewState,
  pinViewDisabledTitle,
  pinResultsForSourceGraph,
  executionStatusForSourceGraph,
  useExecutionStore,
} from "@/features/core/execution";
import { openPinInspectableView } from "@/features/application/execution/openInspectableSource";

/** 将 userValue 转为可显示/编辑的原始值（兼容 DataValue DTO 与本地 raw 格式） */
function toDisplayValue(v: unknown): unknown {
  if (v == null) return v;
  if (typeof v === "object" && !Array.isArray(v) && ("String" in v || "Boolean" in v || "Int64" in v || "Float64" in v || "Null" in v)) {
    return dataValueToRaw(dataValueFromBackend(v as Parameters<typeof dataValueFromBackend>[0]));
  }
  return v;
}

export type PinDragState = "normal" | "highlighted" | "dimmed";

export interface PinProps extends PinModel {
  connected?: boolean;
  linkCount?: number;
  connectionIds?: string[];
  /** 来自 schema 的 pin metaData（如 dropdown 的 widgetOptions） */
  metaData?: PinMetaDataDTO;
  graphPath?: string;
  onPinClick?: (id: string, direction: "input" | "output") => void;
  onPinPointerDown?: (e: React.PointerEvent, pin: PinModel) => void;
  isActive?: boolean;
  pinDragState?: PinDragState;
  onValueChange?: (pinId: string, value: unknown) => void;
  onRemovePin?: (pinId: string) => void;
  forceShowInput?: boolean;
}

const getPinTheme = (
  pin: { type: string; dataType?: PinModel['dataType'] },
  isConnected: boolean,
  baseColor: string,
  containerType?: string,
) => {
  const isExec = isExecPin(pin);
  const isDataFrame = pin.dataType?.kind === 'DataFrame';
  const isStruct = pin.dataType?.kind === 'Struct';
  return {
    isExec,
    isDataFrame,
    isStruct,
    containerType,
    fill: isConnected
      ? baseColor
      : isExec
        ? "rgba(0,0,0,0.1)"
        : "rgba(0,0,0,0.05)",
    stroke: isExec && !isConnected ? "#666" : baseColor,
    strokeWidth: isExec ? 1.5 : 2,
  };
};

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
    onPinClick,
    onPinPointerDown,
    isActive,
    pinDragState = "normal",
    containerType,
    typeDisplay,
    dataType,
    optional,
    defaultValue,
    userValue,
    onValueChange,
    onRemovePin,
    forceShowInput,
  } = props;

  const { t } = useTranslation();
  const { theme: appTheme } = useTheme();
  const isConnected = connected || linkCount > 0 || (isActive ?? false);
  const pinSemantics = useMemo(
    () => ({ type: type ?? 'object', typeDisplay, dataType }),
    [type, typeDisplay, dataType],
  );
  const baseColor = ui?.color ?? getPinTypeColor(pinThemeTypeKey(pinSemantics), appTheme);

  const theme = useMemo(
    () => getPinTheme(pinSemantics, isConnected, baseColor, containerType),
    [pinSemantics, isConnected, baseColor, containerType],
  );

  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);
  const menuActions = useCanvasContextMenuActionsOptional();
  const canRemoveRepeatable = useRepeatablePinRemovable(nodeId, id, graphPath);
  const canRemovePin =
    canRemoveRepeatable && (onRemovePin != null || menuActions?.removeRepeatablePin != null);

  const connectionIds = useGraphDataStore((s) =>
    graphPath ? s.getGraphPinConnections(graphPath, id) : [],
  );
  const executionGraphs = useExecutionStore((s) => s.graphs);
  const pinResults = useMemo(() => {
    if (!graphPath) return undefined;
    const merged = pinResultsForSourceGraph(executionGraphs, graphPath);
    return merged.size > 0 ? merged : undefined;
  }, [executionGraphs, graphPath]);
  const executionStatus = useMemo(
    () => (graphPath ? executionStatusForSourceGraph(executionGraphs, graphPath) : undefined),
    [executionGraphs, graphPath],
  );

  const viewParams = useMemo(
    () =>
      graphPath
        ? buildPinViewParams({
            graphPath: graphPath,
            pinId: id,
            direction,
            isExec: isExecPin(pinSemantics),
            connectionIds,
            pinResults,
            executionStatus,
          })
        : null,
    [graphPath, id, direction, pinSemantics, connectionIds, pinResults, executionStatus],
  );

  const viewState = useMemo(
    () =>
      viewParams
        ? evaluatePinViewState(viewParams)
        : null,
    [viewParams],
  );

  const showViewMenu = viewState?.showMenu ?? false;
  const viewEnabled = viewState?.enabled ?? false;
  const viewDisabledReason = viewState?.disabledReason ?? null;

  const handleRemovePin = useCallback(() => {
    if (onRemovePin) {
      onRemovePin(id);
      return;
    }
    void menuActions?.removeRepeatablePin(nodeId, id);
  }, [onRemovePin, menuActions, nodeId, id]);

  const handleView = useCallback(() => {
    if (!viewParams) return;
    void openPinInspectableView(viewParams, t);
  }, [viewParams, t]);

  const hasLinks = linkCount > 0 || (connectionIds?.length ?? 0) > 0;
  const scalarInputKey = scalarPinInputKey(dataType);
  const canReset =
    direction === "input" &&
    scalarInputKey != null &&
    PRIMITIVE_SCALAR_INPUT_KEYS.has(scalarInputKey) &&
    !containerType &&
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
    !containerType &&
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

  const pinTooltip = `${name} (${pinTypeLabel(pinSemantics)})`;

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
          onContextMenu={handleContextMenu}
          onPointerDown={(e) => {
            if (onPinPointerDown) {
              e.stopPropagation();
              e.preventDefault();
              onPinPointerDown(e, props);
            }
          }}
        >
      {/* Pin Icon Container - 扩大交互区域 */}
      <div
        className={`
          relative w-6 h-6 flex items-center justify-center cursor-crosshair shrink-0 z-20 pin-circle rounded-full
          ${direction === "input" ? "mr-1" : "ml-1"}
          ${contextMenu ? "ring-2 ring-[var(--accent-color)]/60" : ""}
        `}
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
          {theme.isExec ? (
            <>
              <path
                d="M2 2 L7 2 L11 6 L7 10 L2 10 Z"
                fill={theme.fill}
                stroke={theme.stroke}
                strokeWidth={theme.strokeWidth}
                strokeLinejoin="miter"
              />
              {shouldPulse && (
                <path
                  d="M2 2 L7 2 L11 6 L7 10 L2 10 Z"
                  fill="none"
                  stroke={baseColor}
                  strokeWidth={2.5}
                  strokeLinejoin="miter"
                  strokeDasharray="6 24"
                  className="pin-flow-stroke"
                  filter="url(#pinGlow)"
                />
              )}
            </>
          ) : theme.isDataFrame ? (
            <>
              <g>
                <rect x="1.5" y="1.5" width="9" height="9" rx="1" fill={theme.fill} stroke={theme.stroke} strokeWidth={theme.strokeWidth} />
                <line x1="1.5" y1="4.5" x2="10.5" y2="4.5" stroke={theme.stroke} strokeWidth="0.8" />
                <line x1="5" y1="1.5" x2="5" y2="10.5" stroke={theme.stroke} strokeWidth="0.8" />
              </g>
              {shouldPulse && (
                <rect
                  x="1.5" y="1.5" width="9" height="9" rx="1"
                  fill="none"
                  stroke={baseColor}
                  strokeWidth={2.5}
                  strokeDasharray="8 28"
                  className="pin-flow-stroke"
                  filter="url(#pinGlow)"
                />
              )}
            </>
          ) : theme.containerType === "array" ? (
            <>
              <rect
                x="2" y="2" width="8" height="8" rx="1.5"
                fill={theme.fill}
                stroke={theme.stroke}
                strokeWidth={theme.strokeWidth}
              />
              {shouldPulse && (
                <rect
                  x="2" y="2" width="8" height="8" rx="1.5"
                  fill="none"
                  stroke={baseColor}
                  strokeWidth={2.5}
                  strokeDasharray="7 25"
                  className="pin-flow-stroke"
                  filter="url(#pinGlow)"
                />
              )}
            </>
          ) : theme.containerType === "dataseries" ? (
            <>
              <polygon
                points="6,1 11,6 6,11 1,6"
                fill={theme.fill}
                stroke={theme.stroke}
                strokeWidth={theme.strokeWidth}
                strokeLinejoin="miter"
              />
              {shouldPulse && (
                <polygon
                  points="6,1 11,6 6,11 1,6"
                  fill="none"
                  stroke={baseColor}
                  strokeWidth={2.5}
                  strokeLinejoin="miter"
                  strokeDasharray="7 21"
                  className="pin-flow-stroke"
                  filter="url(#pinGlow)"
                />
              )}
            </>
          ) : theme.isStruct ? (
            <>
              <polygon
                points="6,0.5 10.8,3.25 10.8,8.75 6,11.5 1.2,8.75 1.2,3.25"
                fill={theme.fill}
                stroke={theme.stroke}
                strokeWidth={theme.strokeWidth}
                strokeLinejoin="round"
              />
              {shouldPulse && (
                <polygon
                  points="6,0.5 10.8,3.25 10.8,8.75 6,11.5 1.2,8.75 1.2,3.25"
                  fill="none"
                  stroke={baseColor}
                  strokeWidth={2.5}
                  strokeLinejoin="round"
                  strokeDasharray="8 25"
                  className="pin-flow-stroke"
                  filter="url(#pinGlow)"
                />
              )}
            </>
          ) : (
            <>
              <circle
                cx="6" cy="6" r="4.5"
                fill={theme.fill}
                stroke={theme.stroke}
                strokeWidth={theme.strokeWidth}
              />
              {shouldPulse && (
                <circle
                  cx="6" cy="6" r="4.5"
                  fill="none"
                  stroke={baseColor}
                  strokeWidth={2.5}
                  strokeDasharray="7 21"
                  className="pin-flow-stroke"
                  filter="url(#pinGlow)"
                />
              )}
            </>
          )}
          {shouldPulse && (
            <defs>
              <filter id="pinGlow" x="-50%" y="-50%" width="200%" height="200%">
                <feGaussianBlur in="SourceGraphic" stdDeviation="1.5" />
              </filter>
            </defs>
          )}
          {isConnected && !theme.isExec && (
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
