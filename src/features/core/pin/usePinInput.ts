import { useState, useEffect, useCallback } from "react";
import { executeCommand } from "@/features/core/history";

/**
 * Get default value for a pin type.
 * Extracted from PinInput.tsx.
 */
export function getDefaultValue(pinType: string): unknown {
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

/**
 * Pin input logic: value state, save on blur, keyboard handling.
 * Extracted from PinInput.tsx - view should only consume this hook.
 */
export function usePinInput({
  pinId,
  nodeId,
  subgraphId,
  pinType,
  initialValue,
  onValueChange,
}: {
  pinId: string;
  nodeId: string;
  subgraphId: string;
  pinType: string;
  initialValue?: unknown;
  onValueChange?: (value: unknown) => void;
}) {
  const [value, setValue] = useState<unknown>(initialValue ?? getDefaultValue(pinType));
  const [isFocused, setIsFocused] = useState(false);

  useEffect(() => {
    if (initialValue !== undefined) {
      setValue(initialValue);
    }
  }, [initialValue]);

  const handleChange = useCallback(
    (newValue: unknown) => {
      setValue(newValue);
      onValueChange?.(newValue);
    },
    [onValueChange]
  );

  const savePinValue = useCallback(
    async (val?: unknown) => {
      const raw = val !== undefined ? val : value;
      try {
        await executeCommand(
          subgraphId,
          'SetPinValue',
          { pinId, nodeId, pinType, newValue: raw },
          { mergeKey: `pin-value-${pinId}` },
        );
      } catch (error) {
        console.error("[PinInput] Failed to update pin value:", error);
      }
    },
    [subgraphId, nodeId, pinId, pinType, value]
  );

  const handleBlur = useCallback(async () => {
    setIsFocused(false);
    await savePinValue();
  }, [savePinValue, setIsFocused]);

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

  return {
    value,
    isFocused,
    setIsFocused,
    handleChange,
    handleBlur,
    handleKeyDown,
    savePinValue,
  };
}
