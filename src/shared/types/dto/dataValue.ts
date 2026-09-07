import type { SerializedDataValue } from "../domain/dataValue";

const DATA_TYPE_LEAVES = new Set([
  "Boolean",
  "Int64",
  "Float64",
  "String",
  "Date",
  "Datetime",
  "Time",
  "Categorical",
  "Object",
  "DataFrame",
  "Any",
]);

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: Record<string, unknown>, keys: string[]): boolean {
  const actual = Object.keys(value);
  return actual.length === keys.length && keys.every((key) => key in value);
}

function isRustDataTypeWire(value: unknown): boolean {
  if (!isRecord(value) || typeof value.kind !== "string") return false;
  if (DATA_TYPE_LEAVES.has(value.kind)) return hasExactKeys(value, ["kind"]);
  if (value.kind === "Array" || value.kind === "DataSeries") {
    return hasExactKeys(value, ["kind", "inner"]) && isRustDataTypeWire(value.inner);
  }
  if (value.kind === "Struct") {
    return hasExactKeys(value, ["kind", "inner"]) && typeof value.inner === "string";
  }
  return (
    value.kind === "OneOf" &&
    hasExactKeys(value, ["kind", "inner"]) &&
    Array.isArray(value.inner) &&
    value.inner.every(isRustDataTypeWire)
  );
}

function isDataSeriesWire(value: unknown): boolean {
  if (typeof value === "string") return true;
  if (!isRecord(value) || typeof value.id !== "string") return false;
  const allowed = new Set(["id", "elementType", "dummyInfo", "timeSeriesState"]);
  if (Object.keys(value).some((key) => !allowed.has(key))) return false;
  if ("elementType" in value && !isRustDataTypeWire(value.elementType)) return false;
  if ("dummyInfo" in value) {
    const info = value.dummyInfo;
    if (
      !isRecord(info) ||
      !hasExactKeys(info, ["dropCategory", "role"]) ||
      (info.dropCategory !== null && typeof info.dropCategory !== "string") ||
      !["general", "individual", "time"].includes(String(info.role))
    )
      return false;
  }
  return (
    !("timeSeriesState" in value) ||
    value.timeSeriesState === "aligned" ||
    value.timeSeriesState === "unaligned"
  );
}

/** Strict validator for Rust's externally tagged `DataValue` serde wire. */
export function isRustDataValueWire(value: unknown): value is SerializedDataValue {
  if (value === "Null") return true;
  if (!isRecord(value) || Object.keys(value).length !== 1) return false;
  if ("Boolean" in value) return typeof value.Boolean === "boolean";
  if ("Int64" in value) return Number.isSafeInteger(value.Int64);
  if ("Float64" in value)
    return typeof value.Float64 === "number" && Number.isFinite(value.Float64);
  if ("String" in value) return typeof value.String === "string";
  if ("Array" in value) return Array.isArray(value.Array) && value.Array.every(isRustDataValueWire);
  if ("Object" in value) {
    return isRecord(value.Object) && Object.values(value.Object).every(isRustDataValueWire);
  }
  if ("DataFrame" in value) return typeof value.DataFrame === "string";
  if ("DataSeries" in value) return isDataSeriesWire(value.DataSeries);
  if ("Struct" in value) {
    return (
      isRecord(value.Struct) &&
      hasExactKeys(value.Struct, ["typeKey", "handleId"]) &&
      typeof value.Struct.typeKey === "string" &&
      typeof value.Struct.handleId === "string"
    );
  }
  return false;
}
