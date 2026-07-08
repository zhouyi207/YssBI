import React, { useMemo, useCallback } from "react";
import { Pin } from "../Pins/Pin";
import { Pin as PinModel } from "@/shared/types/domain";
import type { UINode } from "@/shared/types/ui";
import { useNodeStyle } from "@/features/core/node";
import { useNodeRegistryStore } from "@/features/core/nodeRegister/useNodeRegistryStore";
import { getPinMetaData } from "@/features/core/pin";
import {
  getRepeatableSlot,
} from "@/features/core/pin/repeatablePinUtils";
import { isPinCompatible } from "@/shared/utils/pinCompatibility";
import { isExecPin } from "@/shared/types/domain/pinSemantics";
import { Button } from "@/components/ui/button";

interface MathNodeLayoutProps {
  node: UINode;
  activePinId?: string | null;
  activePin?: PinModel | null;
  subgraphId?: string;
  onAddInput?: (id: string) => void;
  onRemovePin?: (nodeId: string, pinId: string) => void;
  onPinClick?: (pinId: string, direction: "input" | "output") => void;
  onPinPointerDown?: (e: React.PointerEvent, pin: PinModel) => void;
  onPinValueChange?: (pinId: string, value: unknown) => void;
}

/**
 * Math Node Layout Component
 * 
 * 职责：
 * - 渲染数学节点布局（中心符号 + Pins）
 * - 纯展示组件，样式逻辑在 hooks 中
 */
export const MathNodeLayout: React.FC<MathNodeLayoutProps> = ({
  node,
  activePinId,
  activePin,
  subgraphId,
  onAddInput,
  onRemovePin,
  onPinClick,
  onPinPointerDown,
  onPinValueChange,
}) => {
  const { centerSymbol } = useNodeStyle(node);
  
  const inputsExec = node.inputs.filter(isExecPin);
  const inputsData = node.inputs.filter((p) => !isExecPin(p));
  const outputsExec = node.outputs.filter(isExecPin);
  const outputsData = node.outputs.filter((p) => !isExecPin(p));

  const getPinDragState = useCallback((pin: PinModel): "normal" | "highlighted" | "dimmed" => {
    if (!activePin) return "normal";
    if (pin.id === activePin.id) return "highlighted";
    if (isPinCompatible(pin, activePin)) return "highlighted";
    return "dimmed";
  }, [activePin]);

  const nodeDef = useNodeRegistryStore((s) => s.definitions.get(node.nodeType ?? ""));

  const repeatableSlot = useMemo(() => getRepeatableSlot(nodeDef), [nodeDef]);

  const removePinHandler = onRemovePin
    ? (pinId: string) => onRemovePin(node.id, pinId)
    : undefined;

  return (
    <div className="relative flex flex-col min-h-full">
      {/* Header */}
      <div className="flex items-center justify-between gap-2 px-3 py-1.5 text-sm font-semibold rounded-t border-b border-[var(--node-border)] bg-[var(--node-header-bg)] text-[var(--node-header-fg)]">
        <div className="flex items-center gap-2">
          <span>{node.title}</span>
        </div>
        <div className="text-[10px] opacity-40 font-mono uppercase tracking-tighter">
          {node.category}
        </div>
      </div>

      {/* Center Symbol */}
      {centerSymbol && (
        <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
          <span className="text-2xl font-bold opacity-30 text-black/40">
            {centerSymbol}
          </span>
        </div>
      )}

      {/* Top Row: Flow Pins (Exec) */}
      {(inputsExec.length > 0 || outputsExec.length > 0) && (
        <div className="flex gap-4 px-2 pt-2 z-10 items-start">
          <div className="flex flex-col gap-1 items-start flex-1">
            {inputsExec.map((pin) => (
              <Pin
                key={pin.id}
                {...pin}
                metaData={getPinMetaData(nodeDef, pin.name)}
                subgraphId={subgraphId}
                isActive={activePinId === pin.id}
                pinDragState={getPinDragState(pin)}
                onPinClick={onPinClick}
                onPinPointerDown={onPinPointerDown}
                onValueChange={onPinValueChange}
              />
            ))}
          </div>
          <div className="flex-1" />
          <div className="flex flex-col gap-1 items-end flex-1">
            {outputsExec.map((pin) => (
              <Pin
                key={pin.id}
                {...pin}
                metaData={getPinMetaData(nodeDef, pin.name)}
                subgraphId={subgraphId}
                isActive={activePinId === pin.id}
                pinDragState={getPinDragState(pin)}
                onPinClick={onPinClick}
                onPinPointerDown={onPinPointerDown}
                onValueChange={onPinValueChange}
              />
            ))}
          </div>
        </div>
      )}

      {/* Middle/Bottom Row: Data Pins (Centered) */}
      <div className="flex-1 flex gap-4 px-2 py-2 items-center z-10">
        <div className="flex flex-col gap-1 items-start flex-1">
          {inputsData.map((pin) => (
            <Pin
              key={pin.id}
              {...pin}
              metaData={getPinMetaData(nodeDef, pin.name)}
              subgraphId={subgraphId}
              isActive={activePinId === pin.id}
              pinDragState={getPinDragState(pin)}
              onPinClick={onPinClick}
              onPinPointerDown={onPinPointerDown}
              onValueChange={onPinValueChange}
              onRemovePin={removePinHandler}
            />
          ))}
          {onAddInput && repeatableSlot && (
            <Button
              type="button"
              variant="ghost"
              size="icon-xs"
              onClick={(e) => {
                e.stopPropagation();
                onAddInput?.(node.id);
              }}
              onPointerDown={(e) => e.stopPropagation()}
              className="mt-1 h-4 w-4 text-[10px]"
            >
              +
            </Button>
          )}
        </div>
        <div className="flex-1" />
        <div className="flex flex-col gap-1 items-end flex-1">
          {outputsData.map((pin) => (
            <Pin
              key={pin.id}
              {...pin}
              metaData={getPinMetaData(nodeDef, pin.name)}
              subgraphId={subgraphId}
              isActive={activePinId === pin.id}
              pinDragState={getPinDragState(pin)}
              onPinClick={onPinClick}
              onPinPointerDown={onPinPointerDown}
              onValueChange={onPinValueChange}
            />
          ))}
        </div>
      </div>
    </div>
  );
};
