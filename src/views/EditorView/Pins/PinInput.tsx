import React, { useRef, useState, useEffect, useLayoutEffect, useCallback } from "react";
import { usePinInput } from "@/features/core/pin";

export interface PinInputProps {
  pinId: string;
  nodeId: string;
  subgraphId: string;
  pinType: string;
  value?: unknown;
  onValueChange?: (value: unknown) => void;
}

const INPUT_CLASS = `
  h-[18px] px-1.5 text-[10px] text-[#ccc] leading-[18px] box-border
  bg-[#3c3c3c] border border-transparent rounded-sm
  hover:bg-[#454545]
  focus:bg-[#3c3c3c] focus:border-[#007fd4] focus:outline-none
  transition-colors placeholder-[#5a5a5a]
`;

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

  const isNumeric = pinType === "Int32" || pinType === "Int64" || pinType === "Float32" || pinType === "Float64";

  const [inputText, setInputText] = useState(() =>
    isNumeric ? String(value ?? 0) : ""
  );

  useEffect(() => {
    if (!isFocused && isNumeric) {
      setInputText(String(value ?? 0));
    }
  }, [value, isFocused, isNumeric]);

  const stop = useCallback((e: React.SyntheticEvent) => e.stopPropagation(), []);

  const strText = pinType === "string" ? (value != null ? String(value) : "") : "";
  const measureKey = isNumeric ? inputText : strText;
  const placeholder = pinType === "string" ? "text" : undefined;
  const { ref, width } = useAutoWidth(measureKey, placeholder);

  switch (pinType) {
    case "Int32":
    case "Int64":
      return (
        <input
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

    case "Float32":
    case "Float64":
      return (
        <input
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
        <label
          className="relative inline-flex items-center cursor-pointer ml-0.5"
          onClick={stop}
          onPointerDown={stop}
        >
          <input
            type="checkbox"
            checked={Boolean(value)}
            onChange={async (e) => {
              const newValue = e.target.checked;
              handleChange(newValue);
              savePinValue(newValue);
            }}
            className="sr-only peer"
          />
          <div className="
            w-[26px] h-[14px] rounded-full transition-colors
            bg-[#3c3c3c] peer-checked:bg-[#007fd4]
            after:content-[''] after:absolute after:top-[2px] after:left-[2px]
            after:w-[10px] after:h-[10px] after:rounded-full after:transition-transform
            after:bg-[#ccc] peer-checked:after:translate-x-[12px]
          " />
        </label>
      );

    case "string":
      return (
        <input
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
