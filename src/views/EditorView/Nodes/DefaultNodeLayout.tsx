import React from "react";
import { Pin } from "../Pins/Pin";
import { Pin as PinModel, Node } from "@/shared/types/domain";
import { useVariableStore, useDatabaseStore } from "@/features/core/dataStore";

interface DefaultNodeLayoutProps {
  node: Node & { nodeType?: string; variableId?: string; dataframeId?: string };
  activePinId?: string | null;
  subgraphId?: string;
  onPinClick?: (pinId: string, direction: "input" | "output") => void;
  onPinPointerDown?: (e: React.PointerEvent, pin: PinModel) => void;
  onPinValueChange?: (pinId: string, value: unknown) => void;
}

/** get_variable/set_variable 节点从 variable store 响应式读取显示名 */
function useVariableNodeTitle(
  nodeType: string | undefined,
  variableId: string | undefined,
  fallbackTitle: string
): string {
  const variable = useVariableStore((s) =>
    variableId && (nodeType === "Variables:Get Variable" || nodeType === "Variables:Set Variable")
      ? s.variables[variableId]
      : null
  );
  if (!variable) return fallbackTitle;
  const prefix = nodeType === "Variables:Set Variable" ? "Set " : "Get ";
  return prefix + variable.name;
}

/** get_dataframe 节点从 database store 响应式读取显示名 */
function useDataframeNodeTitle(
  nodeType: string | undefined,
  dataframeId: string | undefined,
  fallbackTitle: string
): string {
  const db = useDatabaseStore((s) =>
    dataframeId && nodeType === "Data:Get DataFrame"
      ? s.databases[dataframeId]
      : null
  );
  if (!db) return fallbackTitle;
  const name = (db as Record<string, unknown>).name as string | undefined;
  return name ? `Get ${name}` : fallbackTitle;
}

/**
 * Default Node Layout Component
 * 
 * 职责：
 * - 渲染默认节点布局（标题 + Pins）
 * - get_variable/set_variable 从 variable store 响应式读取标题，确保刷新后重命名也能更新
 */
export const DefaultNodeLayout: React.FC<DefaultNodeLayoutProps> = ({
  node,
  activePinId,
  subgraphId,
  onPinClick,
  onPinPointerDown,
  onPinValueChange,
}) => {
  const varTitle = useVariableNodeTitle(
    node.nodeType,
    node.variableId,
    node.title
  );
  const displayTitle = useDataframeNodeTitle(
    node.nodeType,
    node.dataframeId,
    varTitle
  );
  const isConstantNode = node.category?.[1] === "Constants";
  const inputsExec = node.inputs.filter(p => p.type === 'exec');
  const inputsData = node.inputs.filter(p => p.type !== 'exec');
  const outputsExec = node.outputs.filter(p => p.type === 'exec');
  const outputsData = node.outputs.filter(p => p.type !== 'exec');

  return (
    <>
      {/* Header */}
      <div className="flex items-center justify-between gap-2 px-3 py-1.5 text-sm font-semibold bg-white/5 rounded-t border-b border-black/20 text-[#cccccc]">
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
            <div className="flex flex-col gap-1 flex-1 items-end">
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
        <div className="flex-1 flex gap-2 px-2 py-2 whitespace-nowrap items-center">
          <div className="flex flex-col gap-1 flex-1">
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
          </div>
          <div className="flex-1" />
          <div className="flex flex-col gap-1 flex-1 items-end">
            {outputsData.map((pin) => (
              <Pin
                key={pin.id}
                {...pin}
                subgraphId={subgraphId}
                isActive={activePinId === pin.id}
                onPinClick={onPinClick}
                onPinPointerDown={onPinPointerDown}
                onValueChange={onPinValueChange}
                forceShowInput={isConstantNode}
              />
            ))}
          </div>
        </div>
      </div>
    </>
  );
};
