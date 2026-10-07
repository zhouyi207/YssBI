import type { PinSemanticCategory, ResolvedThemeTokens } from "@/shared/theme/themeTokens";

const PIN_TYPE_TO_CATEGORY: Record<string, PinSemanticCategory> = {
  Int8: "numeric",
  Int16: "numeric",
  Int32: "numeric",
  Int64: "numeric",
  UInt32: "numeric",
  UInt64: "numeric",
  Float32: "numeric",
  Float64: "numeric",
  int8: "numeric",
  int16: "numeric",
  int32: "numeric",
  int64: "numeric",
  uint32: "numeric",
  uint64: "numeric",
  float: "numeric",
  float32: "numeric",
  float64: "numeric",
  numeric: "numeric",
  bool: "boolean",
  Boolean: "boolean",
  string: "text",
  String: "text",
  date: "temporal",
  Date: "temporal",
  datetime: "temporal",
  DateTime: "temporal",
  Datetime: "temporal",
  time: "temporal",
  Time: "temporal",
  dataframe: "table",
  DataFrame: "table",
  dataseries: "table",
  DataSeries: "table",
  object: "object",
  Object: "object",
  array: "object",
  Array: "object",
  categorical: "object",
  Categorical: "object",
  any: "object",
  Any: "object",
  oneof: "object",
  OneOf: "object",
  struct: "object",
  Struct: "object",
  Null: "object",
};

export function getPinTypeCategory(pinType: string): PinSemanticCategory {
  return PIN_TYPE_TO_CATEGORY[pinType] ?? PIN_TYPE_TO_CATEGORY[pinType.toLowerCase()] ?? "object";
}

export function getPinTypeColor(pinType: string, tokens: ResolvedThemeTokens): string {
  return tokens.pins[getPinTypeCategory(pinType)];
}
