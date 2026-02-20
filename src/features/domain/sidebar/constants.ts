/**
 * Pin type to CSS color variable mapping for sidebar item indicators.
 * Extracted from Sidebar.tsx.
 */
export const PIN_COLORS: Record<string, string> = {
  exec: "var(--exec-color)",
  Int32: "var(--int32-color)",
  Int64: "var(--int64-color)",
  Float32: "var(--float32-color)",
  Float64: "var(--float64-color)",
  Boolean: "var(--bool-color)",
  String: "var(--string-color)",
  Object: "var(--object-color)",
  Array: "var(--array-color)",
  DataFrame: "var(--dataframe-color)",
  dataframe: "var(--dataframe-color)",
  DataSeries: "var(--dataseries-color)",
  dataseries: "var(--dataseries-color)",
  Any: "var(--any-color)",
  any: "var(--any-color)",
  Null: "var(--object-color)",
  int8: "var(--int32-color)",
  int16: "var(--int32-color)",
  int32: "var(--int32-color)",
  int64: "var(--int64-color)",
  uint32: "var(--int32-color)",
  uint64: "var(--int64-color)",
  float: "var(--float64-color)",
  float32: "var(--float32-color)",
  float64: "var(--float64-color)",
  bool: "var(--bool-color)",
  string: "var(--string-color)",
  date: "var(--date-color)",
  datetime: "var(--datetime-color)",
  object: "var(--object-color)",
  array: "var(--array-color)",
  struct: "#0055FF",
  delegate: "#FF3333",
};

/** Sidebar item type icon colors (event, function, macro, variable, data) */
export const TYPE_ICON_COLORS: Record<string, string> = {
  event: "rgba(96, 165, 250, 0.9)",
  function: "rgba(74, 222, 128, 0.9)",
  macro: "rgba(251, 146, 60, 0.9)",
  variable: "rgba(156, 163, 175, 0.85)",
  variableGlobal: "rgba(245, 158, 11, 0.9)",
  data: "rgba(16, 185, 129, 0.9)",
};
