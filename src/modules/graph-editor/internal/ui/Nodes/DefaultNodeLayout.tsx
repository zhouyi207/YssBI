import React, { useCallback } from "react";
import { GraphPinController } from "../Pins/GraphPinController";
import type { PinData } from "@/features/domain/editorProjection/graphRuntimeTypes";
import type { UINode } from "@/features/core/dataStore/nodeView";
import type { GraphContextMenuActions } from "@/features/application/editor";
import { isPinCompatible } from "@/features/domain/editorProjection/connectionRules";

interface DefaultNodeLayoutProps {
  node: UINode;
  activePinId?: string | null;
  activePin?: PinData | null;
  graphPath?: string;
  groupId?: string;
  contextMenuActions?: GraphContextMenuActions | null;
  onPinPointerDown?: (e: React.PointerEvent, pin: PinData) => void;
}

const formatInlineSummary = (value: unknown): string => {
  if (value == null) return "—";
  const rendered = typeof value === "string" ? value : JSON.stringify(value);
  return rendered.length <= 48 ? rendered : `${rendered.slice(0, 47)}…`;
};

/**
 * Default Node Layout Component
 *
 * 职责：
 * - 渲染默认节点布局（标题 + Pins）
 * - 直接渲染后端投影的标题、副标题和 Pin 元数据
 */
export const DefaultNodeLayout: React.FC<DefaultNodeLayoutProps> = ({
  node,
  activePinId,
  activePin,
  graphPath,
  groupId,
  contextMenuActions,
  onPinPointerDown,
}) => {
  const inlineParameters = node.parameterEditors.filter(
    (parameter) => parameter.presentation === "inlineAndDetail",
  );
  const getPinDragState = useCallback(
    (pin: PinData): "normal" | "highlighted" | "dimmed" => {
      if (!activePin) return "normal";
      if (pin.id === activePin.id) return "highlighted";
      if (isPinCompatible(pin, activePin)) return "highlighted";
      return "dimmed";
    },
    [activePin],
  );

  return (
    <>
      {/* Header */}
      <div className="flex items-center gap-3 rounded-t-[5px] border-b border-[var(--node-border)] bg-[var(--node-header-bg)] px-2.5 py-1.5 font-heading text-[12px] font-semibold text-[var(--node-header-fg)]">
        <div className="flex min-w-0 items-center gap-2">
          <span className="truncate tracking-[-0.015em]">{node.title}</span>
          {node.display.userLabel ? (
            <span className="text-[10px] font-normal opacity-70">{node.display.userLabel}</span>
          ) : null}
        </div>
      </div>

      {inlineParameters.length > 0 ? (
        <div className="flex flex-col gap-1 border-b border-[var(--node-border)] px-2 py-1.5">
          {inlineParameters.map((parameter) => (
            <div key={parameter.key} className="flex items-center justify-between gap-2 text-xs">
              <span className="min-w-0 flex-1 truncate">{parameter.display.title}</span>
              <span className="max-w-28 truncate opacity-70">
                {formatInlineSummary(parameter.value)}
              </span>
            </div>
          ))}
        </div>
      ) : null}

      {/* Body */}
      <div className="flex flex-col min-h-[60px]">
        <div className="flex-1 flex gap-2 px-2 py-2 whitespace-nowrap items-center">
          <div className="flex flex-col gap-1 flex-1">
            {node.inputs.map((pin) => {
              const ds = getPinDragState(pin);
              return (
                <GraphPinController
                  key={pin.id}
                  pin={pin}
                  graphPath={graphPath}
                  groupId={groupId}
                  contextMenuActions={contextMenuActions}
                  isActive={activePinId === pin.id}
                  pinDragState={ds}
                  onPinPointerDown={onPinPointerDown}
                />
              );
            })}
          </div>
          <div className="flex-1" />
          <div className="flex flex-col gap-1 flex-1 items-end">
            {node.outputs.map((pin) => {
              const ds = getPinDragState(pin);
              return (
                <GraphPinController
                  key={pin.id}
                  pin={pin}
                  graphPath={graphPath}
                  groupId={groupId}
                  contextMenuActions={contextMenuActions}
                  isActive={activePinId === pin.id}
                  pinDragState={ds}
                  onPinPointerDown={onPinPointerDown}
                />
              );
            })}
          </div>
        </div>
      </div>
    </>
  );
};
