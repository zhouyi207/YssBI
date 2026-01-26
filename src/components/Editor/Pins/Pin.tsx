import React, { useMemo } from "react";
import { Pin as PinModel } from "../Types/nodes";
import { useSchemaStore } from "../Store/useSchemaStore";

export interface PinProps extends PinModel {
  onPinClick?: (id: string, direction: "input" | "output") => void;
  onPinPointerDown?: (e: React.PointerEvent, pin: PinModel) => void;
  isActive?: boolean;
}

// 提取主题逻辑，避免每次渲染都创建新对象
const getPinTheme = (type: string, isConnected: boolean, baseColor: string) => {
  const isExec = type === "exec";
  return {
    isExec,
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
    name,
    type,
    direction,
    links,
    ui,
    onPinClick,
    onPinPointerDown,
    isActive,
  } = props;

  // 从 schema store 获取颜色（getPinColor 已内置默认值）
  const schemaColor = useSchemaStore((s) => s.getPinColor(type));
  const isConnected = links.length > 0 || (isActive ?? false);
  const baseColor = ui?.color ?? schemaColor;

  // 使用 useMemo 缓存主题计算结果
  const theme = useMemo(
    () => getPinTheme(type, isConnected, baseColor),
    [type, isConnected, baseColor]
  );



  return (
    <div
      className={`
       group relative flex items-center h-7 shrink-0 pin-container transition-opacity
        ${direction === "input"
          ? "flex-row justify-start"
          : "flex-row-reverse justify-end"
        }
      `}
      data-pin-id={id}
      title={`${name} (${type})`} // 添加 tooltip 显示类型信息
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
            // 更接近 UE 的五角形 Exec 路径
            <path
              d="M2 2 L7 2 L11 6 L7 10 L2 10 Z"
              fill={theme.fill}
              stroke={theme.stroke}
              strokeWidth={theme.strokeWidth}
              strokeLinejoin="miter"
            />
          ) : (
            <circle
              cx="6"
              cy="6"
              r="4.5"
              fill={theme.fill}
              stroke={theme.stroke}
              strokeWidth={theme.strokeWidth}
            />
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
    </div>
  );
};
