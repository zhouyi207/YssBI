import React, { useRef, useState, useEffect, useLayoutEffect, useCallback } from "react";
import { usePinInput } from "@/features/core/pin";
import { Select } from "@/shared/ui";
import type { DataType, PinMetaDataDTO } from "@/shared/types/domain";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";

export interface PinInputProps {
  pinId: string;
  nodeId: string;
  graphPath: string;
  dataType?: DataType;
  metaData?: PinMetaDataDTO;
  value?: unknown;
  onValueChange?: (value: unknown) => void;
}

const INPUT_CLASS =
  "h-[18px] box-border rounded-sm px-1.5 text-[10px] leading-[18px] placeholder:text-muted-foreground";

const MIN_WIDTH = 28;

function isValidIntInput(s: string): boolean {
  return /^-?\d*$/.test(s);
}

function isValidFloatInput(s: string): boolean {
  return /^-?\d*\.?\d*$/.test(s);
}

function measureInputWidth(el: HTMLInputElement): number {
  const saved = el.style.width;
  el.style.width = "0";
  let sw = el.scrollWidth;

  if (el.placeholder && !el.value) {
    const cs = getComputedStyle(el);
    const canvas = document.createElement("canvas");
    const ctx = canvas.getContext("2d");
    if (ctx) {
      ctx.font = cs.font;
      const phW = Math.ceil(ctx.measureText(el.placeholder).width);
      const pad = (parseFloat(cs.paddingLeft) || 0) + (parseFloat(cs.paddingRight) || 0);
      const bor = (parseFloat(cs.borderLeftWidth) || 0) + (parseFloat(cs.borderRightWidth) || 0);
      sw = Math.max(sw, phW + pad + bor);
    }
  }

  el.style.width = saved;
  return sw;
}

function useAutoWidth(text: string, placeholder?: string) {
  const ref = useRef<HTMLInputElement>(null);
  const [width, setWidth] = useState(MIN_WIDTH);

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const sw = measureInputWidth(el);
    setWidth(Math.max(MIN_WIDTH, sw + 2));
  }, [text, placeholder]);

  return { ref, width };
}

export const PinInput: React.FC<PinInputProps> = ({
  pinId,
  nodeId,
  graphPath,
  dataType,
  metaData,
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
    cancelBlurCommit,
    savePinValue,
    inputKey,
  } = usePinInput({
    pinId,
    nodeId,
    graphPath,
    dataType,
    initialValue,
    onValueChange,
  });

  const isNumeric = inputKey === "Int64" || inputKey === "Float64";

  const [inputText, setInputText] = useState(() => (isNumeric ? String(value ?? 0) : ""));

  useEffect(() => {
    if (!isFocused && isNumeric) {
      setInputText(String(value ?? 0));
    }
  }, [value, isFocused, isNumeric]);

  const stop = useCallback((e: React.SyntheticEvent) => e.stopPropagation(), []);

  const strText = inputKey === "string" ? (value != null ? String(value) : "") : "";
  const measureKey = isNumeric ? inputText : strText;
  const placeholder = inputKey === "string" ? "text" : undefined;
  const { ref, width } = useAutoWidth(measureKey, placeholder);

  const isDropdown =
    metaData?.showWidget &&
    metaData?.widgetType === "dropdown" &&
    (metaData?.widgetOptions?.length ?? 0) > 0;

  if (isDropdown && metaData?.widgetOptions) {
    const strValue =
      inputKey === "string"
        ? value != null
          ? String(value)
          : (metaData.widgetOptions[0] ?? "")
        : String(value ?? "");
    return (
      <div className="min-w-[60px] max-w-[120px]" onClick={stop} onPointerDown={stop}>
        <Select
          value={strValue}
          onChange={(v) => {
            handleChange(v);
            savePinValue(v);
          }}
          options={metaData.widgetOptions}
          className="text-[10px] h-[18px] !w-full"
        />
      </div>
    );
  }

  switch (inputKey) {
    case "Int64":
      return (
        <Input
          ref={ref}
          type="text"
          inputMode="numeric"
          value={inputText}
          onChange={(e) => {
            const raw = e.target.value;
            if (raw === "" || raw === "-") {
              setInputText(raw);
              handleChange(raw);
              return;
            }
            if (!isValidIntInput(raw)) return;
            setInputText(raw);
            const parsed = parseInt(raw, 10);
            if (!isNaN(parsed)) handleChange(parsed);
          }}
          onKeyDown={handleKeyDown}
          onFocus={() => setIsFocused(true)}
          onBlur={() => {
            if (cancelBlurCommit()) {
              setInputText(String(initialValue ?? 0));
              return;
            }
            const parsed = parseInt(inputText, 10);
            const final = isNaN(parsed) ? 0 : parsed;
            handleChange(final);
            setInputText(String(final));
            handleBlur();
          }}
          onClick={stop}
          onPointerDown={stop}
          style={{ width }}
          className={`${INPUT_CLASS} text-center`}
        />
      );

    case "Float64":
      return (
        <Input
          ref={ref}
          type="text"
          inputMode="decimal"
          value={inputText}
          onChange={(e) => {
            const raw = e.target.value;
            if (raw === "" || raw === "-" || raw === "." || raw === "-.") {
              setInputText(raw);
              handleChange(raw);
              return;
            }
            if (!isValidFloatInput(raw)) return;
            setInputText(raw);
            const parsed = parseFloat(raw);
            if (!isNaN(parsed)) handleChange(parsed);
          }}
          onKeyDown={handleKeyDown}
          onFocus={() => setIsFocused(true)}
          onBlur={() => {
            if (cancelBlurCommit()) {
              setInputText(String(initialValue ?? 0));
              return;
            }
            const parsed = parseFloat(inputText);
            const final = isNaN(parsed) ? 0 : parsed;
            handleChange(final);
            setInputText(String(final));
            handleBlur();
          }}
          onClick={stop}
          onPointerDown={stop}
          style={{ width }}
          className={`${INPUT_CLASS} text-center`}
        />
      );

    case "bool":
      return (
        <div className="ml-0.5" onClick={stop} onPointerDown={stop}>
          <Switch
            size="sm"
            checked={Boolean(value)}
            onCheckedChange={(checked) => {
              handleChange(checked);
              savePinValue(checked);
            }}
          />
        </div>
      );

    case "string":
      return (
        <Input
          ref={ref}
          type="text"
          value={strText}
          onChange={(e) => handleChange(e.target.value)}
          onFocus={() => setIsFocused(true)}
          onBlur={handleBlur}
          onKeyDown={handleKeyDown}
          onClick={stop}
          onPointerDown={stop}
          placeholder="text"
          style={{ width }}
          className={INPUT_CLASS}
        />
      );

    default:
      return null;
  }
};
