import type { DeepReadonly } from "@/shared/types/deepReadonly";
/**
 * Data pin runtime semantics — single source for display labels and theme
 * keys. Editor connection compatibility uses the
 * Rust-projected resolved type and never infers data semantics from bare strings.
 */

import type { ValueType } from "./valueType";
import { dataTypeDisplay } from "./valueType";
import type { PortTypeStateDto } from "./editorProjection";

export type PinContainerOverlay = "array" | "dataseries";

export interface PinSemanticsFields {
  typeState: DeepReadonly<PortTypeStateDto>;
}

export function exactPinDataType(pin: PinSemanticsFields): DeepReadonly<ValueType> | undefined {
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
  dataType: DeepReadonly<ValueType> | undefined,
): PinContainerOverlay | undefined {
  if (!dataType) return undefined;
  if (dataType.kind === "Array") return "array";
  if (dataType.kind === "DataSeries") return "dataseries";
  return undefined;
}

/** 容器类型递归到内层标量，返回供固定引脚语义调色板解析的类型别名。 */
export function dataTypeToThemePinType(dt: DeepReadonly<ValueType>): string {
  switch (dt.kind) {
    case "Scalar":
      return {
        Numeric: "numeric",
        Binary: "bool",
        Text: "string",
        Datetime: "datetime",
        Categorical: "categorical",
        Ordinal: "ordinal",
        Identifier: "identifier",
      }[dt.inner];
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
export function scalarPinInputKey(
  type: DeepReadonly<ValueType> | undefined,
): "number" | "bool" | "string" | null {
  if (type?.kind !== "Scalar") return null;
  return type.inner === "Numeric"
    ? "number"
    : type.inner === "Binary"
      ? "bool"
      : type.inner === "Text"
        ? "string"
        : null;
}
