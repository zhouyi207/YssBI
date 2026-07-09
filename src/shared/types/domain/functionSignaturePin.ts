/**
 * Function signature pin — structured DataType contract (single source).
 * `dataType` absent = exec pin; present = data pin.
 */

import type { DataType } from './dataType';
import type { FunctionSignaturePin } from './graph';
import { dataTypeContainerOverlay } from './pinSemantics';

export type SignatureScalarKind = 'Boolean' | 'Int64' | 'Float64' | 'String' | 'Object';

export type SignatureContainerOverlay = import('./pinSemantics').PinContainerOverlay;

/** PinEditor 下拉选项（展示标签）→ 标量 DataType kind */
export const SIGNATURE_EDITOR_TYPE_OPTIONS = [
  'exec',
  'int',
  'float',
  'bool',
  'string',
  'object',
] as const;

export type SignatureEditorTypeOption = (typeof SIGNATURE_EDITOR_TYPE_OPTIONS)[number];

const EDITOR_TO_SCALAR: Record<Exclude<SignatureEditorTypeOption, 'exec'>, SignatureScalarKind> = {
  int: 'Int64',
  float: 'Float64',
  bool: 'Boolean',
  string: 'String',
  object: 'Object',
};

const SCALAR_TO_EDITOR: Record<SignatureScalarKind, Exclude<SignatureEditorTypeOption, 'exec'>> = {
  Boolean: 'bool',
  Int64: 'int',
  Float64: 'float',
  String: 'string',
  Object: 'object',
};

export function isExecSignaturePin(pin: { dataType?: DataType }): boolean {
  return pin.dataType == null;
}

export function signaturePinDataType(pin: { dataType?: DataType }): DataType | undefined {
  return pin.dataType;
}

export function signatureContainerOverlay(
  dataType: DataType | undefined,
): SignatureContainerOverlay | undefined {
  return dataTypeContainerOverlay(dataType);
}

export function signatureScalarKind(dataType: DataType): SignatureScalarKind {
  if (dataType.kind === 'Array' || dataType.kind === 'DataSeries') {
    return signatureScalarKind(dataType.inner);
  }
  if (
    dataType.kind === 'Boolean' ||
    dataType.kind === 'Int64' ||
    dataType.kind === 'Float64' ||
    dataType.kind === 'String' ||
    dataType.kind === 'Object'
  ) {
    return dataType.kind;
  }
  return 'Object';
}

export function buildSignatureDataType(
  scalar: SignatureScalarKind,
  container?: SignatureContainerOverlay,
): DataType {
  const base: DataType = { kind: scalar };
  if (container === 'array') return { kind: 'Array', inner: base };
  if (container === 'dataseries') return { kind: 'DataSeries', inner: base };
  return base;
}

export function signatureEditorTypeOption(pin: {
  dataType?: DataType;
}): SignatureEditorTypeOption {
  if (isExecSignaturePin(pin)) return 'exec';
  return SCALAR_TO_EDITOR[signatureScalarKind(pin.dataType!)];
}

export function applySignatureEditorType(
  pin: { id: string; name: string; dataType?: DataType },
  option: SignatureEditorTypeOption,
): { id: string; name: string; dataType?: DataType } {
  if (option === 'exec') {
    return { id: pin.id, name: pin.name };
  }
  const container = signatureContainerOverlay(pin.dataType);
  return {
    id: pin.id,
    name: pin.name,
    dataType: buildSignatureDataType(EDITOR_TO_SCALAR[option], container),
  };
}

export function cycleSignatureContainer(pin: {
  id: string;
  name: string;
  dataType?: DataType;
}): { id: string; name: string; dataType?: DataType } {
  if (isExecSignaturePin(pin) || !pin.dataType) return pin;
  const scalar = signatureScalarKind(pin.dataType);
  const overlay = signatureContainerOverlay(pin.dataType);
  const next: SignatureContainerOverlay | undefined =
    overlay === undefined ? 'array' : overlay === 'array' ? 'dataseries' : undefined;
  return {
    id: pin.id,
    name: pin.name,
    dataType: buildSignatureDataType(scalar, next),
  };
}

export function createExecSignaturePin(id: string, name: string): FunctionSignaturePin {
  return { id, name };
}

export function createDataSignaturePin(
  id: string,
  name: string,
  dataType: DataType,
): FunctionSignaturePin {
  return { id, name, dataType };
}

export function createDefaultDataSignaturePin(id: string, name: string): FunctionSignaturePin {
  return createDataSignaturePin(id, name, { kind: 'Int64' });
}
