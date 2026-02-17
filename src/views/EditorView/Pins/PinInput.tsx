import React from "react";
import { usePinInput } from "@/features/core/pin";

export interface PinInputProps {
  pinId: string;
  nodeId: string;
  subgraphId: string;
  pinType: string;
  value?: unknown;
  onValueChange?: (value: unknown) => void;
}

/**
 * Pin 输入组件 - 纯展示层，逻辑在 usePinInput 中
 */
export const PinInput: React.FC<PinInputProps> = ({
  pinId,
  nodeId,
  subgraphId,
  pinType,
  value: initialValue,
  onValueChange,
}) => {
  const {
    value,
    isFocused,
    setIsFocused,
    handleChange,
    handleBlur,
    handleKeyDown,
    savePinValue,
  } = usePinInput({
    pinId,
    nodeId,
    subgraphId,
    pinType,
    initialValue,
    onValueChange,
  });

  switch (pinType) {
    case "int":
      return (
        <input
          type="number"
          step="1"
          value={value != null ? Number(value) : 0}
          onChange={(e) => handleChange(parseInt(e.target.value) || 0)}
          onFocus={() => setIsFocused(true)}
          onBlur={handleBlur}
          onKeyDown={handleKeyDown}
          onClick={(e) => e.stopPropagation()}
          onPointerDown={(e) => e.stopPropagation()}
          className={`
            w-16 h-5 px-1 text-[10px] text-center
            bg-black/10 border border-black/20 rounded
            focus:bg-black/20 focus:border-blue-500 focus:outline-none
            transition-colors
            ${isFocused ? "ring-1 ring-blue-500/50" : ""}
          `}
        />
      );

    case "float":
    case "number":
      return (
        <input
          type="number"
          step="0.1"
          value={value != null ? Number(value) : 0}
          onChange={(e) => handleChange(parseFloat(e.target.value) || 0)}
          onFocus={() => setIsFocused(true)}
          onBlur={handleBlur}
          onKeyDown={handleKeyDown}
          onClick={(e) => e.stopPropagation()}
          onPointerDown={(e) => e.stopPropagation()}
          className={`
            w-16 h-5 px-1 text-[10px] text-center
            bg-black/10 border border-black/20 rounded
            focus:bg-black/20 focus:border-blue-500 focus:outline-none
            transition-colors
            ${isFocused ? "ring-1 ring-blue-500/50" : ""}
          `}
        />
      );

    case "bool":
    case "boolean":
      return (
        <input
          type="checkbox"
          checked={Boolean(value)}
          onChange={async (e) => {
            const newValue = e.target.checked;
            handleChange(newValue);
            savePinValue(newValue);
          }}
          onClick={(e) => e.stopPropagation()}
          onPointerDown={(e) => e.stopPropagation()}
          className="w-4 h-4 cursor-pointer"
        />
      );

    case "string":
    case "any":
      return (
        <input
          type="text"
          value={value != null ? String(value) : ""}
          onChange={(e) => handleChange(e.target.value)}
          onFocus={() => setIsFocused(true)}
          onBlur={handleBlur}
          onKeyDown={handleKeyDown}
          onClick={(e) => e.stopPropagation()}
          onPointerDown={(e) => e.stopPropagation()}
          placeholder="text"
          className={`
            w-20 h-5 px-1 text-[10px]
            bg-black/10 border border-black/20 rounded
            focus:bg-black/20 focus:border-blue-500 focus:outline-none
            transition-colors
            ${isFocused ? "ring-1 ring-blue-500/50" : ""}
          `}
        />
      );

    default:
      // 对于其他类型，显示一个通用的文本输入
      return (
        <input
          type="text"
          value={value != null ? (typeof value === "object" ? JSON.stringify(value) : String(value)) : ""}
          onChange={(e) => {
            try {
              const parsed = JSON.parse(e.target.value);
              handleChange(parsed);
            } catch {
              handleChange(e.target.value);
            }
          }}
          onFocus={() => setIsFocused(true)}
          onBlur={handleBlur}
          onKeyDown={handleKeyDown}
          onClick={(e) => e.stopPropagation()}
          onPointerDown={(e) => e.stopPropagation()}
          placeholder="value"
          className={`
            w-20 h-5 px-1 text-[10px]
            bg-black/10 border border-black/20 rounded
            focus:bg-black/20 focus:border-blue-500 focus:outline-none
            transition-colors
            ${isFocused ? "ring-1 ring-blue-500/50" : ""}
          `}
        />
      );
  }
};
