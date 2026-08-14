import React, { useCallback } from "react";
import { useTranslation } from "react-i18next";
import { Pin } from "../Pins/Pin";
import { Pin as PinModel } from "@/shared/types/domain";
import type { UINode } from "@/shared/types/ui";
import { isPinCompatible } from "@/shared/utils/pinCompatibility";
import { isExecPin } from "@/shared/types/domain/pinSemantics";
import { Button } from "@/components/ui/button";
import { DEFAULT_LANGUAGE } from "@/shared/types/settings/LanguageSettings";
import { InlineParameterEditor } from "./InlineParameterEditor";

interface DefaultNodeLayoutProps {
  node: UINode;
  activePinId?: string | null;
  activePin?: PinModel | null;
  graphPath?: string;
  groupId?: string;
  onAddInput?: (id: string) => void;
  onRemovePin?: (nodeId: string, pinId: string) => void;
  onPinClick?: (pinId: string, direction: "input" | "output") => void;
  onPinPointerDown?: (e: React.PointerEvent, pin: PinModel) => void;
  onPinValueChange?: (pinId: string, value: unknown) => void;
}


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
  onAddInput,
  onRemovePin,
  onPinClick,
  onPinPointerDown,
  onPinValueChange,
}) => {
  const { i18n } = useTranslation();
  const locale = i18n.resolvedLanguage || i18n.language || DEFAULT_LANGUAGE;
  const inlineParameters = (node.parameterEditors ?? [])
    .filter((parameter) => parameter.presentation === 'inlineAndDetail');
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
          <span>{node.title}</span>
          {node.display?.userLabel ? (
            <span className="text-[10px] font-normal opacity-70">
              {node.display.userLabel}
            </span>
          ) : null}
        </div>
        <div className="text-[10px] opacity-40 font-mono uppercase tracking-tighter">
          {node.category}
        </div>
      </div>

      {inlineParameters.length > 0 ? (
        <div className="flex flex-col gap-1 border-b border-[var(--node-border)] px-2 py-1.5">
          {inlineParameters.map((parameter) => graphPath ? (
            <InlineParameterEditor
              key={parameter.key}
              graphPath={graphPath}
              nodeId={node.id}
              locale={locale}
              parameter={parameter}
            />
          ) : (
            <div key={parameter.key} className="flex items-center justify-between gap-2 text-xs">
              <span className="truncate">{parameter.display.title}</span>
              <span className="truncate opacity-70">{String(parameter.value ?? '')}</span>
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
                  <Pin
                    key={pin.id}
                    {...pin}
                    graphPath={graphPath}
                    groupId={groupId}
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
                  <Pin
                    key={pin.id}
                    {...pin}
                    graphPath={graphPath}
                    groupId={groupId}
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
                <Pin
                  key={pin.id}
                  {...pin}
                  graphPath={graphPath}
                    groupId={groupId}
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
                <Pin
                  key={pin.id}
                  {...pin}
                  graphPath={graphPath}
                    groupId={groupId}
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
