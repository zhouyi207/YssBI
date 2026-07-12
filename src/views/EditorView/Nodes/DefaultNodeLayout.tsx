import React, { useMemo, useCallback } from "react";
import { Pin } from "../Pins/Pin";
import { Pin as PinModel } from "@/shared/types/domain";
import type { UINode } from "@/shared/types/ui";
import { useVariableStore, useDatabaseStore } from "@/features/core/dataStore";
import { useNodeRegistryStore } from "@/features/core/nodeRegister/useNodeRegistryStore";
import { getPinMetaData } from "@/features/core/pin";
import {
  getRepeatableSlot,
} from "@/features/core/pin/repeatablePinUtils";
import { isPinCompatible } from "@/shared/utils/pinCompatibility";
import { isExecPin } from "@/shared/types/domain/pinSemantics";
import { Button } from "@/components/ui/button";

interface DefaultNodeLayoutProps {
  node: UINode;
  activePinId?: string | null;
  activePin?: PinModel | null;
  graphPath?: string;
  onAddInput?: (id: string) => void;
  onRemovePin?: (nodeId: string, pinId: string) => void;
  onPinClick?: (pinId: string, direction: "input" | "output") => void;
  onPinPointerDown?: (e: React.PointerEvent, pin: PinModel) => void;
  onPinValueChange?: (pinId: string, value: unknown) => void;
}

function isVariableNode(nodeType: string | undefined): boolean {
  return nodeType === "Variables:Get Variable" || nodeType === "Variables:Set Variable";
}

function isDataframeNode(nodeType: string | undefined): boolean {
  return nodeType === "Data:Get DataFrame";
}

/**
 * Default Node Layout Component
 * 
 * 职责：
 * - 渲染默认节点布局（标题 + Pins）
 * - get_variable/set_variable/get_dataframe 标题保持节点语义，资源名显示在 data pin 上
 */
export const DefaultNodeLayout: React.FC<DefaultNodeLayoutProps> = ({
  node,
  activePinId,
  activePin,
  graphPath,
  onAddInput,
  onRemovePin,
  onPinClick,
  onPinPointerDown,
  onPinValueChange,
}) => {
  const variable = useVariableStore((s) =>
    node.variableId && isVariableNode(node.nodeType)
      ? s.variables[node.variableId]
      : null
  );
  const database = useDatabaseStore((s) =>
    node.dataframeId && isDataframeNode(node.nodeType)
      ? s.databases[node.dataframeId]
      : null
  );
  const displayTitle = node.title;
  const isConstantNode = node.category?.[1] === "Constants";
  const resolveResourcePin = useCallback((pin: PinModel): PinModel => {
    if (isExecPin(pin)) return pin;
    if (variable && isVariableNode(node.nodeType)) {
      return { ...pin, name: variable.name };
    }
    if (database && isDataframeNode(node.nodeType)) {
      const name = database.name;
      return name ? { ...pin, name } : pin;
    }
    return pin;
  }, [database, node.nodeType, variable]);

  const inputsExec = node.inputs.filter(isExecPin);
  const inputsData = node.inputs.filter((p) => !isExecPin(p)).map(resolveResourcePin);
  const outputsExec = node.outputs.filter(isExecPin);
  const outputsData = node.outputs.filter((p) => !isExecPin(p)).map(resolveResourcePin);

  const nodeDef = useNodeRegistryStore((s) => s.definitions.get(node.nodeType ?? ""));

  const repeatableSlot = useMemo(() => getRepeatableSlot(nodeDef), [nodeDef]);

  const removePinHandler = onRemovePin
    ? (pinId: string) => onRemovePin(node.id, pinId)
    : undefined;

  const getPinDragState = useCallback((pin: PinModel): "normal" | "highlighted" | "dimmed" => {
    if (!activePin) return "normal";
    if (pin.id === activePin.id) return "highlighted";
    if (isPinCompatible(pin, activePin)) return "highlighted";
    return "dimmed";
  }, [activePin]);

  return (
    <>
      {/* Header */}
      <div
        className="flex items-center justify-between gap-2 px-3 py-1.5 text-sm font-semibold rounded-t border-b border-[var(--node-border)] bg-[var(--node-header-bg)] text-[var(--node-header-fg)]"
      >
        <div className="flex items-center gap-2">
          <span>{displayTitle}</span>
        </div>
        <div className="text-[10px] opacity-40 font-mono uppercase tracking-tighter">
          {node.category}
        </div>
      </div>

      {/* Body */}
      <div className="flex flex-col min-h-[60px]">
        {/* Top Row: Flow Pins (Exec) */}
        {(inputsExec.length > 0 || outputsExec.length > 0) && (
          <div className="flex gap-2 px-2 pt-2 whitespace-nowrap items-start">
            <div className="flex flex-col gap-1 flex-1">
              {inputsExec.map((pin) => {
                const ds = getPinDragState(pin);
                const metaData = getPinMetaData(nodeDef, pin.name);
                return (
                  <Pin
                    key={pin.id}
                    {...pin}
                    metaData={metaData}
                    graphPath={graphPath}
                    isActive={activePinId === pin.id}
                    pinDragState={ds}
                    onPinClick={onPinClick}
                    onPinPointerDown={onPinPointerDown}
                    onValueChange={onPinValueChange}
                    onRemovePin={removePinHandler}
                  />
                );
              })}
            </div>
            <div className="flex-1" />
            <div className="flex flex-col gap-1 flex-1 items-end">
              {outputsExec.map((pin) => {
                const ds = getPinDragState(pin);
                const metaData = getPinMetaData(nodeDef, pin.name);
                return (
                  <Pin
                    key={pin.id}
                    {...pin}
                    metaData={metaData}
                    graphPath={graphPath}
                    isActive={activePinId === pin.id}
                    pinDragState={ds}
                    onPinClick={onPinClick}
                    onPinPointerDown={onPinPointerDown}
                    onValueChange={onPinValueChange}
                    onRemovePin={removePinHandler}
                  />
                );
              })}
            </div>
          </div>
        )}

        {/* Middle/Bottom Row: Data Pins (Centered) */}
        <div className="flex-1 flex gap-2 px-2 py-2 whitespace-nowrap items-center">
          <div className="flex flex-col gap-1 flex-1">
            {inputsData.map((pin) => {
              const ds = getPinDragState(pin);
              const metaData = getPinMetaData(nodeDef, pin.name);
              return (
                <Pin
                  key={pin.id}
                  {...pin}
                  metaData={metaData}
                  graphPath={graphPath}
                  isActive={activePinId === pin.id}
                  pinDragState={ds}
                  onPinClick={onPinClick}
                  onPinPointerDown={onPinPointerDown}
                  onValueChange={onPinValueChange}
                  onRemovePin={removePinHandler}
                />
              );
            })}
          </div>
          <div className="flex-1" />
          <div className="flex flex-col gap-1 flex-1 items-end">
            {outputsData.map((pin) => {
              const ds = getPinDragState(pin);
              const metaData = getPinMetaData(nodeDef, pin.name);
              return (
                <Pin
                  key={pin.id}
                  {...pin}
                  metaData={metaData}
                  graphPath={graphPath}
                  isActive={activePinId === pin.id}
                  pinDragState={ds}
                  onPinClick={onPinClick}
                  onPinPointerDown={onPinPointerDown}
                  onValueChange={onPinValueChange}
                  onRemovePin={removePinHandler}
                  forceShowInput={isConstantNode}
                />
              );
            })}
          </div>
        </div>

        {repeatableSlot && onAddInput && (
          <div className="flex justify-end px-2 pb-2">
            <Button
              type="button"
              variant="ghost"
              size="icon-xs"
              onClick={(e) => {
                e.stopPropagation();
                onAddInput(node.id);
              }}
              onPointerDown={(e) => e.stopPropagation()}
              className="h-4 w-4 text-[10px]"
            >
              +
            </Button>
          </div>
        )}
      </div>
    </>
  );
};
