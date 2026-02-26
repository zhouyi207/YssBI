import React, { useMemo, useState, useCallback } from "react";
import { Pin as PinModel } from "@/shared/types/domain";
import { useTheme } from "@/features/core/theme/useTheme";
import { getPinTypeColor } from "@/features/core/theme/pinTypeTheme";
import { PinInput } from "./PinInput";
import { PinContextMenu } from "../ContextMenu";
import { dataValueFromBackend } from "@/shared/types/dto/dataValue";
import { dataValueToRaw } from "@/shared/types/domain/dataValue";

/** 将 userValue 转为可显示/编辑的原始值（兼容 DataValue DTO 与本地 raw 格式） */
function toDisplayValue(v: unknown): unknown {
  if (v == null) return v;
  if (typeof v === "object" && !Array.isArray(v) && ("String" in v || "Boolean" in v || "Int32" in v || "Int64" in v || "Float32" in v || "Float64" in v || "Null" in v)) {
    return dataValueToRaw(dataValueFromBackend(v));
  }
  return v;
}

const PRIMITIVE_PIN_TYPES = new Set(["bool", "Int32", "Int64", "Float32", "Float64", "string"]);

export type PinDragState = "normal" | "highlighted" | "dimmed";

export interface PinProps extends PinModel {
  subgraphId?: string;
  onPinClick?: (id: string, direction: "input" | "output") => void;
  onPinPointerDown?: (e: React.PointerEvent, pin: PinModel) => void;
  isActive?: boolean;
  pinDragState?: PinDragState;
  onValueChange?: (pinId: string, value: unknown) => void;
  removable?: boolean;
  onRemovePin?: (pinId: string) => void;
  forceShowInput?: boolean;
}

const getPinTheme = (type: string, isConnected: boolean, baseColor: string, containerType?: string) => {
  const isExec = type === "exec";
  const isDataFrame = type === "dataframe";
  const isStruct = type === "struct";
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
    links,
    ui,
    subgraphId,
    onPinClick,
    onPinPointerDown,
    isActive,
    pinDragState = "normal",
    containerType,
    typeDisplay,
    optional,
    defaultValue,
    userValue,
    onValueChange,
    removable,
    onRemovePin,
    forceShowInput,
  } = props;

  const { theme: appTheme } = useTheme();
  const isConnected = links.length > 0 || (isActive ?? false);
  const baseColor = ui?.color ?? getPinTypeColor(type ?? "any", appTheme);

  const theme = useMemo(
    () => getPinTheme(type, isConnected, baseColor, containerType),
    [type, isConnected, baseColor, containerType]
  );

  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);

  const handleContextMenu = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      e.stopPropagation();
      setContextMenu({ x: e.clientX, y: e.clientY });
    },
    []
  );

  const shouldPulse = !optional && !isConnected && direction === "input" && type !== "exec";

  const showInput =
    (!isConnected || forceShowInput === true) &&
    PRIMITIVE_PIN_TYPES.has(type) &&
    !containerType &&
    (direction === "input" || forceShowInput === true) &&
    subgraphId &&
    nodeId;

  const dragStyle: React.CSSProperties | undefined =
    pinDragState === "dimmed"
      ? { opacity: 0.25, transition: "opacity 150ms, filter 150ms" }
      : pinDragState === "highlighted"
        ? { filter: "brightness(1.25) saturate(1.4)", transition: "opacity 150ms, filter 150ms" }
        : undefined;

  return (
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
      title={`${name} (${typeDisplay ?? type})`}
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
          relative w-6 h-6 flex items-center justify-center cursor-crosshair shrink-0 z-20 pin-circle
          ${direction === "input" ? "mr-1" : "ml-1"}
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
          {isConnected && (
            <circle
              cx={theme.isExec ? "5" : "6"}
              cy="6"
              r="1.2"
              fill="white"
              className="pointer-events-none"
            />
          )}
        </svg>
      </div>

      {/* Label - 增加 hover 效果 */}
      <span
        className={`
          text-[10px] font-bold select-none uppercase tracking-wider px-1 z-10 pointer-events-none
          transition-colors
          ${isConnected ? "text-gray-900" : "text-gray-500"}
          group-hover:text-black
        `}
      >
        {name}
      </span>

      {showInput && (
        <PinInput
          pinId={id}
          nodeId={nodeId}
          subgraphId={subgraphId}
          pinType={type}
          value={toDisplayValue(userValue ?? defaultValue)}
          onValueChange={(value) => onValueChange?.(id, value)}
        />
      )}

      {contextMenu && (
        <PinContextMenu
          position={contextMenu}
          removable={removable && !!onRemovePin}
          onRemove={() => onRemovePin?.(id)}
          onClose={() => setContextMenu(null)}
        />
      )}
    </div>
  );
};
