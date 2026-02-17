import React from "react";
import { Pin } from "../Pins/Pin";
import { Pin as PinModel } from "@/shared/types/domain";
import type { Node } from "@/shared/types/ui";
import { useNodeStyle } from "@/features/core/node";

interface MathNodeLayoutProps {
  node: Node;
  activePinId?: string | null;
  subgraphId?: string;
  onAddInput?: (id: string) => void;
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
  subgraphId,
  onAddInput,
  onPinClick,
  onPinPointerDown,
  onPinValueChange,
}) => {
  const { centerSymbol } = useNodeStyle(node);
  
  const inputsExec = node.inputs.filter(p => p.type === 'exec');
  const inputsData = node.inputs.filter(p => p.type !== 'exec');
  const outputsExec = node.outputs.filter(p => p.type === 'exec');
  const outputsData = node.outputs.filter(p => p.type !== 'exec');

  return (
    <div className="relative flex flex-col min-h-full">
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
                subgraphId={subgraphId}
                isActive={activePinId === pin.id}
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
                subgraphId={subgraphId}
                isActive={activePinId === pin.id}
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
              subgraphId={subgraphId}
              isActive={activePinId === pin.id}
              onPinClick={onPinClick}
              onPinPointerDown={onPinPointerDown}
              onValueChange={onPinValueChange}
            />
          ))}
          {onAddInput && (
            <button
              onClick={(e) => {
                e.stopPropagation();
                onAddInput?.(node.id);
              }}
              onPointerDown={(e) => e.stopPropagation()}
              className="mt-1 w-4 h-4 flex items-center justify-center bg-black/10 hover:bg-black/20 rounded text-[10px]"
            >
              +
            </button>
          )}
        </div>
        <div className="flex-1" />
        <div className="flex flex-col gap-1 items-end flex-1">
          {outputsData.map((pin) => (
            <Pin
              key={pin.id}
              {...pin}
              subgraphId={subgraphId}
              isActive={activePinId === pin.id}
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
