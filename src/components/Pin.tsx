import React from "react";
import { Pin as PinModel } from "./node/models";

export interface PinProps extends PinModel {
  onPinClick?: (id: string, direction: "input" | "output") => void;
  onPinPointerDown?: (e: React.PointerEvent, pin: PinModel) => void;
}

const PIN_COLORS: Record<string, string> = {
  exec: "#ffffff",
  int: "#3b82f6",
  float: "#3b82f6",
  bool: "#f64146",
  string: "#10b981",
  object: "#8b5cf6",
  array: "#ef4444",
  struct: "#f97316",
  delegate: "#ec4899",
};

export const Pin: React.FC<PinProps> = (props) => {
  const {
    id,
    name,
    type,
    direction,
    links,
    ui,
    onPinClick,
    onPinPointerDown,
  } = props;
  const isConnected = links.length > 0;
  const baseColor = ui?.color ?? PIN_COLORS[type] ?? "#9ca3af";

  // 确保在浅色背景下 exec 针脚可见
  const strokeColor =
    type === "exec" ? (isConnected ? baseColor : "#444") : baseColor;
  const fillColor = isConnected
    ? baseColor
    : type === "exec"
    ? "rgba(0,0,0,0.1)"
    : "rgba(0,0,0,0.05)";

  return (
    <div
      className={`
        relative flex items-center h-7 shrink-0 pin-container
        ${
          direction === "input"
            ? "flex-row justify-start"
            : "flex-row-reverse justify-end"
        }
      `}
      data-pin-id={id}
      onPointerDown={(e) => {
        if (onPinPointerDown) {
          e.stopPropagation();
          e.preventDefault();
          onPinPointerDown(e, props);
        }
      }}
    >
      {/* Pin Icon Container */}
      <div
        className={`
          relative w-5 h-5 flex items-center justify-center cursor-pointer shrink-0 z-20 pin-circle
          ${direction === "input" ? "mr-2" : "ml-2"}
        `}
        onClick={(e) => {
          e.stopPropagation();
          onPinClick?.(id, direction);
        }}
      >
        <svg
          width="14"
          height="14"
          viewBox="0 0 12 12"
          className="overflow-visible"
          style={{ display: "block" }}
        >
          {type === "exec" ? (
            <path
              d="M 2,1 L 10,6 L 2,11 Z"
              style={{
                fill: fillColor,
                stroke: strokeColor,
                strokeWidth: "1.5px",
                strokeLinejoin: "round",
              }}
            />
          ) : (
            <circle
              cx="6"
              cy="6"
              r="4.5"
              style={{
                fill: fillColor,
                stroke: baseColor,
                strokeWidth: "2px",
              }}
            />
          )}
          {isConnected && (
            <circle
              cx={type === "exec" ? "4.5" : "6"}
              cy="6"
              r="1.2"
              style={{ fill: "white" }}
              className="pointer-events-none"
            />
          )}
        </svg>
      </div>

      {/* Label */}
      <span className="text-[11px] text-gray-900 font-black select-none uppercase tracking-tight px-1 z-10 pointer-events-none">
        {name}
      </span>
    </div>
  );
};
