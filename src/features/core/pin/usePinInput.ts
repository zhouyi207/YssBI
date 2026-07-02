import { useState, useEffect, useCallback, useRef } from "react";
import { executeCommand } from "@/features/core/history";
import { logger } from '@/utils/appLogger';

/**
 * Get default value for a pin type.
 * Extracted from PinInput.tsx.
 */
export function getDefaultValue(pinType: string): unknown {
  switch (pinType) {
    case "Int64":
      return 0;
    case "Float64":
      return 0;
    case "bool":
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
  const skipNextBlurSaveRef = useRef(false);

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
        logger.graph.error(`Failed to update pin value: ${error instanceof Error ? error.message : String(error)}`, 'PinInput');
      }
    },
    [subgraphId, nodeId, pinId, pinType, value]
  );

  const cancelBlurCommit = useCallback(() => {
    if (!skipNextBlurSaveRef.current) return false;
    skipNextBlurSaveRef.current = false;
    setIsFocused(false);
    return true;
  }, []);

  const handleBlur = useCallback(async () => {
    if (cancelBlurCommit()) return;
    setIsFocused(false);
    await savePinValue();
  }, [cancelBlurCommit, savePinValue, setIsFocused]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        (e.currentTarget as HTMLElement).blur();
      } else if (e.key === "Escape") {
        const restored = initialValue ?? getDefaultValue(pinType);
        skipNextBlurSaveRef.current = true;
        setValue(restored);
        onValueChange?.(restored);
        (e.currentTarget as HTMLElement).blur();
      }
    },
    [initialValue, onValueChange, pinType]
  );

  return {
    value,
    isFocused,
    setIsFocused,
    handleChange,
    handleBlur,
    handleKeyDown,
    cancelBlurCommit,
    savePinValue,
  };
}
