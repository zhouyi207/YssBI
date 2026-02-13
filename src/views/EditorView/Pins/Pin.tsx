import React, { useMemo } from "react";
import { Pin as PinModel } from "@/shared/types/editor";
import { PinInput } from "./PinInput";

export interface PinProps extends PinModel {
  subgraphId?: string;
  onPinClick?: (id: string, direction: "input" | "output") => void;
  onPinPointerDown?: (e: React.PointerEvent, pin: PinModel) => void;
  isActive?: boolean;
  onValueChange?: (pinId: string, value: any) => void;
}

// 提取主题逻辑，避免每次渲染都创建新对象
const getPinTheme = (type: string, isConnected: boolean, baseColor: string, isArray?: boolean) => {
  const isExec = type === "exec";
  return {
    isExec,
    isArray: !!isArray,
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
    isArray,
    defaultValue,
    userValue,  // 🆕 添加 userValue
    onValueChange,
  } = props;

  // 颜色逻辑已完全迁移至前端主题系统
  // 从 CSS 变量获取颜色，或者使用 ui?.color 覆盖
  const isConnected = links.length > 0 || (isActive ?? false);
  const baseColor = ui?.color ?? `var(--${type}-color, #CCCCCC)`;

  // 使用 useMemo 缓存主题计算结果
  const theme = useMemo(
    () => getPinTheme(type, isConnected, baseColor, isArray),
    [type, isConnected, baseColor, isArray]
  );

  // 判断是否显示输入控件
  // 条件：输入 Pin、数据类型（非 exec）、未连接、有 subgraphId
  const showInput =
    direction === "input" &&
    type !== "exec" &&
    !isConnected &&
    subgraphId &&
    nodeId;

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
          ) : theme.isArray ? (
            // 数组/List 显示为圆角矩形
            <rect
              x="2"
              y="2"
              width="8"
              height="8"
              rx="1.5"
              fill={theme.fill}
              stroke={theme.stroke}
              strokeWidth={theme.strokeWidth}
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

      {/* 输入控件 - 仅在未连接的输入数据 Pin 上显示 */}
      {showInput && (
        <div className="ml-1">
          <PinInput
            pinId={id}
            nodeId={nodeId}
            subgraphId={subgraphId}
            pinType={type}
            value={userValue ?? defaultValue}  // 🆕 优先使用 userValue
            onValueChange={(value) => onValueChange?.(id, value)}
          />
        </div>
      )}
    </div>
  );
};
