import type { ThemeSettings } from "@/shared/types/settings";

/** Pin 类型到 ThemeSettings 键的显式映射（与后端 data_type_to_pin_type 一致） */
const PIN_TYPE_TO_THEME_KEY: Record<string, keyof ThemeSettings> = {
  exec: "execColor",
  Int32: "int32Color",
  Int64: "int64Color",
  Float32: "float32Color",
  Float64: "float64Color",
  bool: "boolColor",
  Boolean: "boolColor",
  string: "stringColor",
  String: "stringColor",
  object: "objectColor",
  Object: "objectColor",
  array: "arrayColor",
  Array: "arrayColor",
  dataframe: "dataframeColor",
  DataFrame: "dataframeColor",
  dataseries: "dataseriesColor",
  DataSeries: "dataseriesColor",
  date: "dateColor",
  Date: "dateColor",
  datetime: "datetimeColor",
  DateTime: "datetimeColor",
  Datetime: "datetimeColor",
  time: "datetimeColor",
  Time: "datetimeColor",
  categorical: "categoricalColor",
  Categorical: "categoricalColor",
  any: "anyColor",
  Any: "anyColor",
  oneof: "oneofColor",
  OneOf: "oneofColor",
  struct: "structColor",
  Struct: "structColor",
};

/** 获取 pin 类型对应的主题颜色键 */
export function getPinTypeThemeKey(pinType: string): keyof ThemeSettings {
  const key = PIN_TYPE_TO_THEME_KEY[pinType];
  if (key) return key;
  return "anyColor";
}

/** 获取 pin 类型对应的颜色值 */
export function getPinTypeColor(pinType: string, theme: ThemeSettings): string {
  const key = getPinTypeThemeKey(pinType);
  return theme[key] ?? theme.connectionLines ?? "#6b6b6b";
}
