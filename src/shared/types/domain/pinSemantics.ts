/**
 * Pin runtime semantics — single source for exec/data discrimination,
 * display labels, and theme keys. Type compatibility uses `dataType` via
 * `buildPinDataType` in pinCompatibility; never infer from bare `type` strings.
 */

import type { DataType } from './dataType';
import { dataTypeDisplay } from './dataType';

export type PinFlowKind = 'Exec' | 'Data';

export type PinContainerOverlay = 'array' | 'dataseries';

export interface PinSemanticsFields {
  type: string;
  dataType?: DataType;
}

export function isExecPin(pin: Pick<PinSemanticsFields, 'type'>): boolean {
  return pin.type === 'exec';
}

export function pinFlowKind(pin: Pick<PinSemanticsFields, 'type'>): PinFlowKind {
  return isExecPin(pin) ? 'Exec' : 'Data';
}

/** UI label only — not used for compatibility or coercion. */
export function pinTypeLabel(pin: PinSemanticsFields): string {
  if (isExecPin(pin)) return 'exec';
  if (pin.dataType) return dataTypeDisplay(pin.dataType);
  return 'unknown';
}

/** Array / DataSeries 容器叠加层（签名编辑与 pin 视觉共用）。 */
export function dataTypeContainerOverlay(
  dataType: DataType | undefined,
): PinContainerOverlay | undefined {
  if (!dataType) return undefined;
  if (dataType.kind === 'Array') return 'array';
  if (dataType.kind === 'DataSeries') return 'dataseries';
  return undefined;
}

/** 容器类型递归到内层标量，映射 ThemeSettings 色键。 */
export function dataTypeToThemePinType(dt: DataType): string {
  switch (dt.kind) {
    case 'Boolean':
      return 'bool';
    case 'Int64':
      return 'Int64';
    case 'Float64':
      return 'Float64';
    case 'String':
      return 'string';
    case 'Date':
      return 'date';
    case 'Datetime':
      return 'datetime';
    case 'Time':
      return 'time';
    case 'Categorical':
      return 'categorical';
    case 'Array':
      return dataTypeToThemePinType(dt.inner);
    case 'Object':
      return 'object';
    case 'Any':
      return 'any';
    case 'DataFrame':
      return 'dataframe';
    case 'DataSeries':
      return dataTypeToThemePinType(dt.inner);
    case 'Struct':
      return 'struct';
    case 'OneOf':
      return 'oneof';
  }
}

/** Scalar pin input widget key, or null when the pin is not an editable scalar. */
export function scalarPinInputKey(dataType: DataType | undefined): string | null {
  if (!dataType) return null;
  switch (dataType.kind) {
    case 'Boolean':
      return 'bool';
    case 'Int64':
      return 'Int64';
    case 'Float64':
      return 'Float64';
    case 'String':
      return 'string';
    default:
      return null;
  }
}

export const PRIMITIVE_SCALAR_INPUT_KEYS = new Set(['bool', 'Int64', 'Float64', 'string']);
