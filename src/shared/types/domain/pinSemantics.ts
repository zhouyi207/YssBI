/**
 * Data pin runtime semantics — single source for display labels and theme
 * keys. Editor connection compatibility uses the
 * Rust-projected resolved type and never infers data semantics from bare strings.
 */

import type { DataType } from "./dataType";
import { dataTypeDisplay } from "./dataType";
import type { PortTypeStateDto } from "./editorProjection";

export type PinContainerOverlay = "array" | "dataseries";

export interface PinSemanticsFields {
  typeState: PortTypeStateDto;
}

export function exactPinDataType(pin: PinSemanticsFields): DataType | undefined {
  return pin.typeState.status === "exact" ? (pin.typeState.dataType ?? undefined) : undefined;
}

/** UI label only — not used for compatibility or coercion. */
export function pinTypeLabel(pin: PinSemanticsFields): string {
  const dataType = exactPinDataType(pin);
  if (dataType) {
    return dataTypeDisplay(dataType);
  }
  if (pin.typeState.status === "constrained") {
    return pin.typeState.domain.map(dataTypeDisplay).join(" | ");
  }
  return "unknown";
}

/** Array / DataSeries 容器叠加层（签名编辑与 pin 视觉共用）。 */
export function dataTypeContainerOverlay(
  dataType: DataType | undefined,
): PinContainerOverlay | undefined {
  if (!dataType) return undefined;
  if (dataType.kind === "Array") return "array";
  if (dataType.kind === "DataSeries") return "dataseries";
  return undefined;
}

/** 容器类型递归到内层标量，返回供固定引脚语义调色板解析的类型别名。 */
export function dataTypeToThemePinType(dt: DataType): string {
  switch (dt.kind) {
    case "Boolean":
      return "bool";
    case "Int64":
      return "Int64";
    case "Float64":
      return "Float64";
    case "String":
      return "string";
    case "Date":
      return "date";
    case "Datetime":
      return "datetime";
    case "Time":
      return "time";
    case "Categorical":
      return "categorical";
    case "Array":
      return dataTypeToThemePinType(dt.inner);
    case "Object":
      return "object";
    case "Any":
      return "any";
    case "DataFrame":
      return "dataframe";
    case "DataSeries":
      return dataTypeToThemePinType(dt.inner);
    case "Struct":
      return "struct";
    case "OneOf":
      return "oneof";
  }
}

/** Scalar pin input widget key, or null when the pin is not an editable scalar. */
export function scalarPinInputKey(dataType: DataType | undefined): string | null {
  if (!dataType) return null;
  switch (dataType.kind) {
    case "Boolean":
      return "bool";
    case "Int64":
      return "Int64";
    case "Float64":
      return "Float64";
    case "String":
      return "string";
    default:
      return null;
  }
}

export const PRIMITIVE_SCALAR_INPUT_KEYS = new Set(["bool", "Int64", "Float64", "string"]);
