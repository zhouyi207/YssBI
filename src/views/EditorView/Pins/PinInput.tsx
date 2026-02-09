import React, { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useNodeStore } from "@/features/node-registry/stores";

export interface PinInputProps {
  pinId: string;
  nodeId: string;
  subgraphId: string;
  pinType: string;
  value?: any;
  onValueChange?: (value: any) => void;
}

/**
 * Pin 输入组件
 * 根据 Pin 类型显示不同的输入控件
 */
export const PinInput: React.FC<PinInputProps> = ({
  pinId,
  nodeId,
  subgraphId,
  pinType,
  value: initialValue,
  onValueChange,
}) => {
  const [value, setValue] = useState<any>(initialValue ?? getDefaultValue(pinType));
  const [isFocused, setIsFocused] = useState(false);
  const updateNode = useNodeStore((state) => state.updateNode);

  useEffect(() => {
    if (initialValue !== undefined) {
      setValue(initialValue);
    }
  }, [initialValue]);

  const handleChange = useCallback(
    (newValue: any) => {
      setValue(newValue);
      onValueChange?.(newValue);
    },
    [onValueChange]
  );

  const handleBlur = useCallback(async () => {
    setIsFocused(false);
    
    // 调用后端 API 更新 Pin 值
    try {
      console.log("[PinInput] Saving value:", {
        subgraphId,
        nodeId,
        pinId,
        value,
        pinType
      });
      
      await invoke("update_pin_user_value", {
        subgraphId,
        nodeId,
        pinId,
        value,
      });
      
      console.log("[PinInput] Value saved successfully to backend");
      
      // 🆕 同时更新前端 store
      updateNode(subgraphId, nodeId, (node) => {
        const cloned = node.clone();
        // 更新输入 pin 的 userValue
        const inputPin = cloned.inputs.find((p) => p.id === pinId);
        if (inputPin) {
          inputPin.userValue = value;
          console.log("[PinInput] Updated frontend store for input pin:", pinId);
        }
        // 也检查输出 pin（虽然通常不会有输入控件）
        const outputPin = cloned.outputs.find((p) => p.id === pinId);
        if (outputPin) {
          outputPin.userValue = value;
          console.log("[PinInput] Updated frontend store for output pin:", pinId);
        }
        return cloned;
      });
      
      console.log("[PinInput] Frontend store updated successfully");
    } catch (error) {
      console.error("[PinInput] Failed to update pin value:", error);
    }
  }, [subgraphId, nodeId, pinId, value, pinType, updateNode]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        (e.currentTarget as HTMLElement).blur();
      } else if (e.key === "Escape") {
        setValue(initialValue ?? getDefaultValue(pinType));
        (e.currentTarget as HTMLElement).blur();
      }
    },
    [initialValue, pinType]
  );

  // 根据类型渲染不同的输入控件
  switch (pinType) {
    case "int":
      return (
        <input
          type="number"
          step="1"
          value={value ?? 0}
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
          value={value ?? 0}
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
          checked={value ?? false}
          onChange={async (e) => {
            const newValue = e.target.checked;
            handleChange(newValue);
            // 立即保存布尔值
            try {
              await invoke("update_pin_user_value", {
                subgraphId,
                nodeId,
                pinId,
                value: newValue,
              });
              console.log("[PinInput] Boolean value saved to backend");
              
              // 🆕 同时更新前端 store
              updateNode(subgraphId, nodeId, (node) => {
                const cloned = node.clone();
                const inputPin = cloned.inputs.find((p) => p.id === pinId);
                if (inputPin) {
                  inputPin.userValue = newValue;
                }
                const outputPin = cloned.outputs.find((p) => p.id === pinId);
                if (outputPin) {
                  outputPin.userValue = newValue;
                }
                return cloned;
              });
              console.log("[PinInput] Boolean value updated in frontend store");
            } catch (error) {
              console.error("[PinInput] Failed to update boolean value:", error);
            }
          }}
          onClick={(e) => e.stopPropagation()}
          onPointerDown={(e) => e.stopPropagation()}
          className="w-4 h-4 cursor-pointer"
        />
      );

    case "string":
    case "any":  // 🆕 添加 any 类型支持
      return (
        <input
          type="text"
          value={value ?? ""}
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
          value={typeof value === "object" ? JSON.stringify(value) : value ?? ""}
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

/**
 * 根据类型获取默认值
 */
function getDefaultValue(pinType: string): any {
  switch (pinType) {
    case "int":
    case "float":
    case "number":
      return 0;
    case "bool":
    case "boolean":
      return false;
    case "string":
      return "";
    default:
      return null;
  }
}
