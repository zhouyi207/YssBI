import type { ValueType } from "./valueType";
import { getDefaultValue } from "./valueType";

/** Shared Rust literal wire, used by constants, port literals and node defaults. */
export type SerializedDataValue =
  | "Null"
  | { Bool: boolean }
  | { Integer: string }
  | { Unsigned: string }
  | { Decimal: string }
  | { String: string }
  | { Bytes: number[] }
  | { List: SerializedDataValue[] }
  | { Object: Record<string, SerializedDataValue> };

/** Transient editor fields. Table text is parsed by Graph before it is committed. */
export type DataValue =
  | { kind: "Boolean"; value: boolean }
  | { kind: "Int64" | "UInt64" | "Float64"; value: string }
  | { kind: "String" | "DataFrame" | "DataSeries"; value: string }
  | { kind: "Bytes"; value: number[] }
  | { kind: "Array"; value: DataValue[] }
  | { kind: "Object"; value: Record<string, DataValue> }
  | { kind: "Null" };

export function deserializeDataValue(value: SerializedDataValue): DataValue {
  if (value === "Null") return { kind: "Null" };
  if ("Bool" in value) return { kind: "Boolean", value: value.Bool };
  if ("Integer" in value) return { kind: "Int64", value: value.Integer };
  if ("Unsigned" in value) return { kind: "UInt64", value: value.Unsigned };
  if ("Decimal" in value) return { kind: "Float64", value: value.Decimal };
  if ("String" in value) return { kind: "String", value: value.String };
  if ("Bytes" in value) return { kind: "Bytes", value: value.Bytes };
  if ("List" in value) return { kind: "Array", value: value.List.map(deserializeDataValue) };
  return {
    kind: "Object",
    value: Object.fromEntries(
      Object.entries(value.Object).map(([key, child]) => [key, deserializeDataValue(child)]),
    ),
  };
}

export function dataValueToRaw(value: DataValue): unknown {
  switch (value.kind) {
    case "Null":
      return null;
    case "Int64":
    case "UInt64": {
      const numeric = Number(value.value);
      return Number.isSafeInteger(numeric) ? numeric : value.value;
    }
    case "Float64":
      return Number(value.value);
    case "Array":
      return value.value.map(dataValueToRaw);
    case "Object":
      return Object.fromEntries(
        Object.entries(value.value).map(([key, child]) => [key, dataValueToRaw(child)]),
      );
    default:
      return value.value;
  }
}

function numericValue(raw: number | string): DataValue {
  const text = String(raw).trim() || "0";
  const match = /^([+-]?)(\d*)(?:\.(\d*))?(?:[eE]([+-]?\d+))?$/.exec(text);
  if (!match || !(match[2] || match[3]) || !Number.isFinite(Number(text))) {
    throw new TypeError("Invalid numeric literal");
  }
  const digits = match[2] + (match[3] ?? "");
  const point = match[2].length + Number(match[4] ?? 0);
  let integer = point <= 0 ? "0" : digits.slice(0, point).padEnd(point, "0");
  let fraction = point < 0 ? "0".repeat(-point) + digits : digits.slice(point);
  integer = integer.replace(/^0+(?=\d)/, "");
  fraction = fraction.replace(/0+$/, "");
  const number =
    (match[1] === "-" && (integer !== "0" || fraction) ? "-" : "") +
    integer +
    (fraction ? `.${fraction}` : "");
  if (!fraction) {
    const value = BigInt(number);
    if (value >= -(1n << 63n) && value < 1n << 63n) return { kind: "Int64", value: number };
    if (value >= 0n && value < 1n << 64n) return { kind: "UInt64", value: number };
  }
  return { kind: "Float64", value: number };
}

export function inferDataValueFromJson(raw: unknown): DataValue {
  if (raw == null) return { kind: "Null" };
  if (typeof raw === "boolean") return { kind: "Boolean", value: raw };
  if (typeof raw === "number") return numericValue(raw);
  if (typeof raw === "string") return { kind: "String", value: raw };
  if (Array.isArray(raw)) return { kind: "Array", value: raw.map(inferDataValueFromJson) };
  if (typeof raw === "object")
    return {
      kind: "Object",
      value: Object.fromEntries(
        Object.entries(raw).map(([key, value]) => [key, inferDataValueFromJson(value)]),
      ),
    };
  throw new TypeError("Invalid literal");
}

export function dataValueFromRaw(raw: unknown, dataType: ValueType): DataValue {
  raw ??= getDefaultValue(dataType);
  if (raw == null) return { kind: "Null" };
  if (dataType.kind === "Scalar") {
    if (dataType.inner === "Numeric") return numericValue(String(raw));
    if (dataType.inner === "Text" || dataType.inner === "Datetime") {
      return { kind: "String", value: String(raw) };
    }
  }
  if (dataType.kind === "Array")
    return {
      kind: "Array",
      value: Array.isArray(raw) ? raw.map((value) => dataValueFromRaw(value, dataType.inner)) : [],
    };
  if (dataType.kind === "DataFrame" || dataType.kind === "DataSeries") {
    return typeof raw === "string" && raw.trim()
      ? { kind: dataType.kind, value: raw }
      : { kind: "Null" };
  }
  return inferDataValueFromJson(raw);
}

export function serializeDataValue(value: DataValue): SerializedDataValue {
  switch (value.kind) {
    case "Null":
      return "Null";
    case "Boolean":
      return { Bool: value.value };
    case "Int64":
      return { Integer: value.value };
    case "UInt64":
      return { Unsigned: value.value };
    case "Float64":
      return { Decimal: value.value };
    case "String":
    case "DataFrame":
    case "DataSeries":
      return { String: value.value };
    case "Bytes":
      return { Bytes: value.value };
    case "Array":
      return { List: value.value.map(serializeDataValue) };
    case "Object":
      return {
        Object: Object.fromEntries(
          Object.entries(value.value).map(([key, child]) => [key, serializeDataValue(child)]),
        ),
      };
  }
}
