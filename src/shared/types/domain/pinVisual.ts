/**
 * Pin visual spec — single source for shape, color key, and edge semantics.
 * Consumed by Pin rendering, connection lines, and edge overlays.
 */

import {
  dataTypeContainerOverlay,
  dataTypeToThemePinType,
  exactPinDataType,
  pinTypeLabel,
  type PinSemanticsFields,
} from "./pinSemantics";

export type PinShape = "circle" | "diamond" | "roundedRect" | "gridRect" | "hexagon";

export interface PinVisualInput extends PinSemanticsFields {}

export interface PinVisualSpec {
  label: string;
  shape: PinShape;
  colorKey: string;
  container?: import("./pinSemantics").PinContainerOverlay;
  dashedStroke: boolean;
}

export interface PinRenderStyle {
  fill: string;
  stroke: string;
  strokeWidth: number;
}

function resolveShape(pin: PinVisualInput): PinShape {
  const dataType = exactPinDataType(pin);
  if (dataType?.kind === "DataFrame") return "gridRect";
  if (dataType?.kind === "Struct") return "hexagon";

  const container = dataTypeContainerOverlay(dataType);
  if (container === "array") return "roundedRect";
  if (container === "dataseries") return "diamond";
  return "circle";
}

export function resolvePinVisualSpec(pin: PinVisualInput): PinVisualSpec {
  const dataType = exactPinDataType(pin);
  const colorKey = dataType ? dataTypeToThemePinType(dataType) : "object";
  const container = dataTypeContainerOverlay(dataType);

  return {
    label: pinTypeLabel(pin),
    shape: resolveShape(pin),
    colorKey,
    container,
    dashedStroke: dataType?.kind === "OneOf",
  };
}

export function resolvePinRenderStyle(
  isConnected: boolean,
  baseColor: string,
  mutedForeground: string,
): PinRenderStyle {
  return {
    fill: isConnected ? baseColor : "color-mix(in srgb, var(--muted-foreground) 5%, transparent)",
    stroke: isConnected ? baseColor : mutedForeground,
    strokeWidth: 2,
  };
}
