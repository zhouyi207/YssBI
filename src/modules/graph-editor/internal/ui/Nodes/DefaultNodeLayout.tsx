import React, { useCallback } from "react";
import { GraphPinController } from "../Pins/GraphPinController";
import { Pin as PinModel } from "@/shared/types/domain";
import type { UINode } from "@/features/core/dataStore/nodeView";
import type { GraphContextMenuActions } from "@/features/application/editor";
import { isPinCompatible } from "@/features/domain/editorProjection/connectionRules";
import { isExecPin } from "@/shared/types/domain/pinSemantics";
import { Button } from "@/components/ui/button";

interface DefaultNodeLayoutProps {
  node: UINode;
  activePinId?: string | null;
  activePin?: PinModel | null;
  graphPath?: string;
  groupId?: string;
  contextMenuActions?: GraphContextMenuActions | null;
  onAddInput?: (id: string) => void;
  onRemovePin?: (nodeId: string, pinId: string) => void;
  onPinClick?: (pinId: string, direction: "input" | "output") => void;
  onPinPointerDown?: (e: React.PointerEvent, pin: PinModel) => void;
  onPinValueChange?: (pinId: string, value: unknown) => void;
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
  onAddInput,
  onRemovePin,
  onPinClick,
  onPinPointerDown,
  onPinValueChange,
}) => {
  const inlineParameters = (node.parameterEditors ?? []).filter(
    (parameter) => parameter.presentation === "inlineAndDetail",
  );
  const inputsExec = node.inputs.filter(isExecPin);
  const inputsData = node.inputs.filter((p) => !isExecPin(p));
  const outputsExec = node.outputs.filter(isExecPin);
  const outputsData = node.outputs.filter((p) => !isExecPin(p));

  const hasRepeatableInput = node.inputs.some(
    (pin) => !isExecPin(pin) && pin.instanceKind === "userCreated",
  );

  const removePinHandler = onRemovePin
    ? (pinId: string) => {
        const pin = [...node.inputs, ...node.outputs].find((candidate) => candidate.id === pinId);
        if (pin?.canRemove) onRemovePin(node.id, pinId);
      }
    : undefined;

  const getPinDragState = useCallback(
    (pin: PinModel): "normal" | "highlighted" | "dimmed" => {
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
      <div className="flex items-center justify-between gap-3 rounded-t-[5px] border-b border-[var(--node-border)] bg-[var(--node-header-bg)] px-2.5 py-1.5 font-heading text-[12px] font-semibold text-[var(--node-header-fg)]">
        <div className="flex min-w-0 items-center gap-2">
          <span className="truncate tracking-[-0.015em]">{node.title}</span>
          {node.display?.userLabel ? (
            <span className="text-[10px] font-normal opacity-70">{node.display.userLabel}</span>
          ) : null}
        </div>
        <div className="shrink-0 font-mono text-[8px] font-medium uppercase tracking-[0.08em] opacity-45">
          {node.category}
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
        {/* Top Row: Flow Pins (Exec) */}
        {(inputsExec.length > 0 || outputsExec.length > 0) && (
          <div className="flex gap-2 px-2 pt-2 whitespace-nowrap items-start">
            <div className="flex flex-col gap-1 flex-1">
              {inputsExec.map((pin) => {
                const ds = getPinDragState(pin);
                return (
                  <GraphPinController
                    key={pin.id}
                    {...pin}
                    graphPath={graphPath}
                    groupId={groupId}
                    contextMenuActions={contextMenuActions}
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
                return (
                  <GraphPinController
                    key={pin.id}
                    {...pin}
                    graphPath={graphPath}
                    groupId={groupId}
                    contextMenuActions={contextMenuActions}
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
              return (
                <GraphPinController
                  key={pin.id}
                  {...pin}
                  graphPath={graphPath}
                  groupId={groupId}
                  contextMenuActions={contextMenuActions}
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
              return (
                <GraphPinController
                  key={pin.id}
                  {...pin}
                  graphPath={graphPath}
                  groupId={groupId}
                  contextMenuActions={contextMenuActions}
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

        {hasRepeatableInput && onAddInput && (
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
