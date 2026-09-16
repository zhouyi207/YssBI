/**
 * Domain Types - DataValue
 *
 * 数据值定义及领域内辅助函数
 */

import type { ValueType } from "./valueType";
import { dataTypeFromKey, getDefaultValue, isBackendDataType } from "./valueType";

export type DataSeriesValuePayload = {
  id: string;
  elementType?: import("./valueType").ValueType;
};

/**
 * 数据值（可辨识联合，与 ValueType 对应）
 */
export type DataValue =
  | { kind: "Boolean"; value: boolean }
  | { kind: "Int64"; value: number }
  | { kind: "Float64"; value: number }
  | { kind: "String"; value: string }
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
      elementType?: ValueType;
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

function dataTypeFromBackend(value: unknown): ValueType {
  if (isBackendDataType(value)) return value;
  if (typeof value === "string") return dataTypeFromKey(value);
  return { kind: "Any" };
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

/** 从原始值 + ValueType 创建 DataValue */
export function dataValueFromRaw(raw: unknown, dataType: ValueType): DataValue {
  const def = getDefaultValue(dataType);
  if (raw === null || raw === undefined) {
    return rawToDataValue(def, dataType);
  }
  return rawToDataValue(raw, dataType);
}

function rawToDataValue(raw: unknown, dataType: ValueType): DataValue {
  if (dataType.kind === "Scalar") {
    if (raw == null) return { kind: "Null" };
    if (dataType.inner === "Numeric") {
      const value = Number(raw);
      return Number.isSafeInteger(value) ? { kind: "Int64", value } : { kind: "Float64", value };
    }
    if (dataType.inner === "Binary")
      return typeof raw === "boolean"
        ? { kind: "Boolean", value: raw }
        : inferDataValueFromJson(raw);
    if (dataType.inner === "Text" || dataType.inner === "Datetime")
      return { kind: "String", value: String(raw) };
    return inferDataValueFromJson(raw);
  }
  switch (dataType.kind) {
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
              ? { elementType: payload.elementType as import("./valueType").ValueType }
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
