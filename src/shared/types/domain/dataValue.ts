/**
 * Domain Types - DataValue
 *
 * 数据值定义及领域内辅助函数
 */

import type { DataType } from "./dataType";
import { dataTypeFromKey, getDefaultValue } from "./dataType";

export type DataSeriesValuePayload = {
  id: string;
  elementType?: import("./dataType").DataType;
};

/**
 * 数据值（可辨识联合，与 DataType 对应）
 */
export type DataValue =
  | { kind: "Boolean"; value: boolean }
  | { kind: "Int64"; value: number }
  | { kind: "Float64"; value: number }
  | { kind: "String"; value: string }
  | { kind: "Date"; value: string }
  | { kind: "Datetime"; value: string }
  | { kind: "Time"; value: string }
  | { kind: "Categorical"; value: string }
  | { kind: "Array"; value: DataValue[] }
  | { kind: "Object"; value: Record<string, unknown> }
  | { kind: "DataFrame"; value: string }
  | { kind: "DataSeries"; value: DataSeriesValuePayload | string }
  | { kind: "Struct"; value: { typeKey: string; handleId: string } }
  | { kind: "Null" };

export type SerializedDataSeriesValue =
  | string
  | {
      id: string;
      elementType?: DataType;
      dummyInfo?: unknown;
      timeSeriesState?: unknown;
    };

/** 后端 DataValue 序列化格式（Rust serde 外部标签枚举） */
export type SerializedDataValue =
  | { Boolean: boolean }
  | { Int64: number }
  | { Float64: number }
  | { String: string }
  | { Array: SerializedDataValue[] }
  | { Object: Record<string, SerializedDataValue> }
  | { DataFrame: string }
  | { DataSeries: SerializedDataSeriesValue }
  | { Struct: { typeKey: string; handleId: string } }
  | "Null";

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function dataTypeFromBackend(value: unknown): DataType {
  if (typeof value === "string") return dataTypeFromKey(value);
  if (!isRecord(value) || typeof value.kind !== "string") return { kind: "Any" };
  if (value.kind === "Array" && value.inner !== undefined) {
    return { kind: "Array", inner: dataTypeFromBackend(value.inner) };
  }
  if (value.kind === "DataSeries" && value.inner !== undefined) {
    return { kind: "DataSeries", inner: dataTypeFromBackend(value.inner) };
  }
  if (value.kind === "OneOf" && Array.isArray(value.inner)) {
    return { kind: "OneOf", inner: value.inner.map(dataTypeFromBackend) };
  }
  if (value.kind === "Struct" && typeof value.inner === "string") {
    return { kind: "Struct", inner: value.inner };
  }
  return dataTypeFromKey(value.kind);
}

export function deserializeDataValue(value: unknown): DataValue {
  if (value == null || value === "Null") return { kind: "Null" };
  if (!isRecord(value)) return { kind: "Null" };
  if ("kind" in value && "value" in value) return value as DataValue;
  if ("Boolean" in value && typeof value.Boolean === "boolean") {
    return { kind: "Boolean", value: value.Boolean };
  }
  if ("Int64" in value && typeof value.Int64 === "number") {
    return { kind: "Int64", value: value.Int64 };
  }
  if ("Float64" in value && typeof value.Float64 === "number") {
    return { kind: "Float64", value: value.Float64 };
  }
  if ("String" in value && typeof value.String === "string") {
    return { kind: "String", value: value.String };
  }
  if ("Array" in value && Array.isArray(value.Array)) {
    return { kind: "Array", value: value.Array.map(deserializeDataValue) };
  }
  if ("Object" in value && isRecord(value.Object)) {
    return {
      kind: "Object",
      value: Object.fromEntries(
        Object.entries(value.Object).map(([key, value]) => [
          key,
          dataValueToRaw(deserializeDataValue(value)),
        ]),
      ),
    };
  }
  if ("DataFrame" in value && typeof value.DataFrame === "string") {
    return { kind: "DataFrame", value: value.DataFrame };
  }
  if ("DataSeries" in value) {
    if (typeof value.DataSeries === "string") {
      return { kind: "DataSeries", value: value.DataSeries };
    }
    if (isRecord(value.DataSeries) && typeof value.DataSeries.id === "string") {
      return {
        kind: "DataSeries",
        value: {
          id: value.DataSeries.id,
          ...(value.DataSeries.elementType === undefined
            ? {}
            : { elementType: dataTypeFromBackend(value.DataSeries.elementType) }),
        },
      };
    }
  }
  if (
    "Struct" in value &&
    isRecord(value.Struct) &&
    typeof value.Struct.typeKey === "string" &&
    typeof value.Struct.handleId === "string"
  ) {
    return { kind: "Struct", value: value.Struct as { typeKey: string; handleId: string } };
  }
  return inferDataValueFromJson(value);
}

/** 从 DataValue 提取原始值（用于 UI 显示/编辑） */
export function dataValueToRaw(dv: DataValue): unknown {
  switch (dv.kind) {
    case "Boolean":
    case "Int64":
    case "Float64":
    case "String":
    case "Date":
    case "Datetime":
    case "Time":
    case "Categorical":
    case "DataFrame":
      return dv.value;
    case "DataSeries":
      return typeof dv.value === "string" ? dv.value : dv.value.id;
    case "Struct":
      return dv.value;
    case "Array":
      return dv.value.map(dataValueToRaw);
    case "Object":
      return dv.value;
    case "Null":
      return null;
  }
}

/** 从 JSON 原始值推断 DataValue（用于 Array<Any> 等无法静态确定元素类型的场景） */
export function inferDataValueFromJson(raw: unknown): DataValue {
  if (raw === null || raw === undefined) return { kind: "Null" };
  if (typeof raw === "boolean") return { kind: "Boolean", value: raw };
  if (typeof raw === "number") {
    return Number.isInteger(raw) ? { kind: "Int64", value: raw } : { kind: "Float64", value: raw };
  }
  if (typeof raw === "string") return { kind: "String", value: raw };
  if (Array.isArray(raw)) {
    return { kind: "Array", value: raw.map(inferDataValueFromJson) };
  }
  if (typeof raw === "object") {
    return { kind: "Object", value: raw as Record<string, unknown> };
  }
  return { kind: "Null" };
}

/** 从原始值 + DataType 创建 DataValue */
export function dataValueFromRaw(raw: unknown, dataType: DataType): DataValue {
  const def = getDefaultValue(dataType);
  if (raw === null || raw === undefined) {
    return rawToDataValue(def, dataType);
  }
  return rawToDataValue(raw, dataType);
}

function rawToDataValue(raw: unknown, dataType: DataType): DataValue {
  const k = dataType.kind;
  switch (k) {
    case "Boolean":
      return { kind: "Boolean", value: Boolean(raw) };
    case "Int64":
      return { kind: "Int64", value: Math.floor(Number(raw)) };
    case "Float64":
      return { kind: "Float64", value: Number(raw) };
    case "String":
      return { kind: "String", value: String(raw) };
    case "Date":
      return { kind: "Date", value: raw != null ? String(raw) : "" };
    case "Datetime":
      return { kind: "Datetime", value: raw != null ? String(raw) : "" };
    case "Time":
      return { kind: "Time", value: raw != null ? String(raw) : "" };
    case "Categorical":
      return { kind: "Categorical", value: raw != null ? String(raw) : "" };
    case "Array":
      return {
        kind: "Array",
        value: Array.isArray(raw)
          ? raw.map((x) =>
              dataType.inner?.kind === "Any" || !dataType.inner
                ? inferDataValueFromJson(x)
                : dataValueFromRaw(x, dataType.inner),
            )
          : [],
      };
    case "Object":
      return {
        kind: "Object",
        value:
          raw && typeof raw === "object" && !Array.isArray(raw)
            ? (raw as Record<string, unknown>)
            : {},
      };
    case "DataFrame":
      return { kind: "DataFrame", value: String(raw ?? "") };
    case "DataSeries":
      if (typeof raw === "string") {
        return raw.trim() ? { kind: "DataSeries", value: raw } : { kind: "Null" };
      }
      if (raw && typeof raw === "object" && "id" in raw) {
        const payload = raw as { id?: unknown; elementType?: unknown };
        const id = typeof payload.id === "string" ? payload.id : "";
        if (!id.trim()) return { kind: "Null" };
        return {
          kind: "DataSeries",
          value: {
            id,
            ...(payload.elementType
              ? { elementType: payload.elementType as import("./dataType").DataType }
              : {}),
          },
        };
      }
      return { kind: "Null" };
    case "Struct":
      if (raw && typeof raw === "object" && "handleId" in raw) {
        return {
          kind: "Struct",
          value: { typeKey: dataType.inner, handleId: String(raw.handleId) },
        };
      }
      return { kind: "Null" };
    case "Any":
      return inferDataValueFromJson(raw);
    default:
      return { kind: "Null" };
  }
}

export function serializeDataValue(dv: DataValue): SerializedDataValue {
  switch (dv.kind) {
    case "Boolean":
      return { Boolean: dv.value };
    case "Int64":
      return { Int64: dv.value };
    case "Float64":
      return { Float64: dv.value };
    case "String":
      return { String: dv.value };
    // 后端 DataValue 无 Date/Datetime/Time/Categorical 变体，统一以 String 承载
    case "Date":
    case "Datetime":
    case "Time":
    case "Categorical":
      return { String: dv.value };
    case "Array":
      return { Array: dv.value.map(serializeDataValue) };
    case "Object":
      return {
        Object: Object.fromEntries(
          Object.entries(dv.value).map(([key, value]) => [
            key,
            serializeDataValue(inferDataValueFromJson(value)),
          ]),
        ),
      };
    case "DataFrame":
      return { DataFrame: dv.value };
    case "DataSeries": {
      if (typeof dv.value === "string") {
        return { DataSeries: dv.value };
      }
      const payload: SerializedDataSeriesValue = { id: dv.value.id };
      if (dv.value.elementType) {
        payload.elementType = dv.value.elementType;
      }
      return { DataSeries: payload };
    }
    case "Struct":
      return { Struct: dv.value };
    case "Null":
      return "Null";
  }
}
