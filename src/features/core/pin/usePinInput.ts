import { useState, useEffect, useCallback, useRef } from "react";
import { executeCommand } from "@/features/core/history";
import { logger } from "@/features/core/observability/logger";
import type { DataType } from "@/shared/types/domain/dataType";
import { scalarPinInputKey } from "@/shared/types/domain/pinSemantics";

/**
 * Get default value for a scalar pin dataType.
 */
function getDefaultValue(dataType: DataType | undefined): unknown {
  const key = scalarPinInputKey(dataType);
  switch (key) {
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
 */
export function usePinInput({
  pinId,
  nodeId,
  graphPath,
  dataType,
  initialValue,
}: {
  pinId: string;
  nodeId: string;
  graphPath: string;
  dataType?: DataType;
  initialValue?: unknown;
}) {
  const inputKey = scalarPinInputKey(dataType);
  const [value, setValue] = useState<unknown>(initialValue ?? getDefaultValue(dataType));
  const [isFocused, setIsFocused] = useState(false);
  const skipNextBlurSaveRef = useRef(false);

  useEffect(() => {
    if (initialValue !== undefined) {
      setValue(initialValue);
    }
  }, [initialValue]);

  const handleChange = useCallback((newValue: unknown) => setValue(newValue), []);

  const savePinValue = useCallback(
    async (val?: unknown) => {
      const raw = val !== undefined ? val : value;
      const applied = await executeCommand(graphPath, "SetPinValue", {
        pinId,
        nodeId,
        newValue: raw,
      });
      if (!applied) logger.graph.error("Failed to update port value", "PinInput");
    },
    [graphPath, nodeId, pinId, value],
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
        const restored = initialValue ?? getDefaultValue(dataType);
        skipNextBlurSaveRef.current = true;
        setValue(restored);
        (e.currentTarget as HTMLElement).blur();
      }
    },
    [initialValue, dataType],
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
    inputKey,
  };
}
