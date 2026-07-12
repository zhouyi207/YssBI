/**
 * Pin visual spec — single source for shape, color key, and edge semantics.
 * Consumed by Pin rendering, connection lines, and edge overlays.
 */

import {
  dataTypeContainerOverlay,
  dataTypeToThemePinType,
  isExecPin,
  pinTypeLabel,
  type PinSemanticsFields,
} from './pinSemantics';

export type PinShape =
  | 'exec'
  | 'circle'
  | 'diamond'
  | 'roundedRect'
  | 'gridRect'
  | 'hexagon';

export type PinEdgeKind = 'exec' | 'data';

export interface PinVisualInput extends PinSemanticsFields {}

export interface PinVisualSpec {
  label: string;
  shape: PinShape;
  colorKey: string;
  container?: import('./pinSemantics').PinContainerOverlay;
  edgeKind: PinEdgeKind;
  dashedStroke: boolean;
}

export interface PinRenderStyle {
  fill: string;
  stroke: string;
  strokeWidth: number;
}

function resolveShape(pin: PinVisualInput): PinShape {
  if (isExecPin(pin)) return 'exec';
  if (pin.dataType?.kind === 'DataFrame') return 'gridRect';
  if (pin.dataType?.kind === 'Struct') return 'hexagon';

  const container = dataTypeContainerOverlay(pin.dataType);
  if (container === 'array') return 'roundedRect';
  if (container === 'dataseries') return 'diamond';
  return 'circle';
}

export function resolvePinVisualSpec(pin: PinVisualInput): PinVisualSpec {
  if (isExecPin(pin)) {
    return {
      label: pinTypeLabel(pin),
      shape: 'exec',
      colorKey: 'exec',
      edgeKind: 'exec',
      dashedStroke: false,
    };
  }

  const colorKey = pin.dataType ? dataTypeToThemePinType(pin.dataType) : pin.type;
  const container = dataTypeContainerOverlay(pin.dataType);

  return {
    label: pinTypeLabel(pin),
    shape: resolveShape(pin),
    colorKey,
    container,
    edgeKind: 'data',
    dashedStroke: pin.dataType?.kind === 'OneOf',
  };
}

export function resolvePinRenderStyle(
  spec: PinVisualSpec,
  isConnected: boolean,
  baseColor: string,
): PinRenderStyle {
  const isExec = spec.edgeKind === 'exec';
  return {
    fill: isConnected
      ? baseColor
      : isExec
        ? 'rgba(0,0,0,0.1)'
        : 'rgba(0,0,0,0.05)',
    stroke: isExec && !isConnected ? '#666' : baseColor,
    strokeWidth: isExec ? 1.5 : 2,
  };
}
