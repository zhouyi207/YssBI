/**
 * Pin type to CSS color variable mapping for sidebar item indicators.
 * Extracted from Sidebar.tsx.
 */
export const PIN_COLORS: Record<string, string> = {
  exec: "var(--pin-exec)",
  Int32: "var(--pin-numeric)",
  Int64: "var(--pin-numeric)",
  Float32: "var(--pin-numeric)",
  Float64: "var(--pin-numeric)",
  Boolean: "var(--pin-boolean)",
  String: "var(--pin-text)",
  Object: "var(--pin-object)",
  Array: "var(--pin-object)",
  DataFrame: "var(--pin-table)",
  dataframe: "var(--pin-table)",
  DataSeries: "var(--pin-table)",
  dataseries: "var(--pin-table)",
  Any: "var(--pin-object)",
  any: "var(--pin-object)",
  Null: "var(--pin-object)",
  int8: "var(--pin-numeric)",
  int16: "var(--pin-numeric)",
  int32: "var(--pin-numeric)",
  int64: "var(--pin-numeric)",
  uint32: "var(--pin-numeric)",
  uint64: "var(--pin-numeric)",
  float: "var(--pin-numeric)",
  float32: "var(--pin-numeric)",
  float64: "var(--pin-numeric)",
  bool: "var(--pin-boolean)",
  string: "var(--pin-text)",
  date: "var(--pin-temporal)",
  datetime: "var(--pin-temporal)",
  time: "var(--pin-temporal)",
  object: "var(--pin-object)",
  array: "var(--pin-object)",
  struct: "var(--pin-object)",
  delegate: "var(--status-danger)",
};

/** Sidebar item type icon colors (event, function, variable, data) */
export const TYPE_ICON_COLORS: Record<string, string> = {
  event: "var(--status-info)",
  function: "var(--status-success)",
  variable: "var(--muted-foreground)",
  variableGlobal: "var(--status-warning)",
  data: "var(--status-success)",
  worksheet: "var(--pin-temporal)",
};
