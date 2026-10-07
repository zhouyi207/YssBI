/**
 * Function signature pin — structured data-only contract.
 */

import type { ValueType } from "./valueType";
import type { FunctionSignaturePin } from "./graph";
import { dataTypeContainerOverlay } from "./pinSemantics";

import { SEMANTIC_TYPES, type SemanticType } from "./database";
export type SignatureScalarKind = SemanticType | "Object" | "DataFrame";
export type SignatureContainerOverlay = import("./pinSemantics").PinContainerOverlay;
export const SIGNATURE_EDITOR_TYPE_OPTIONS = [...SEMANTIC_TYPES, "DataFrame"] as const;
export type SignatureEditorTypeOption = SignatureScalarKind;

export function signatureContainerOverlay(
  dataType: ValueType,
): SignatureContainerOverlay | undefined {
  return dataTypeContainerOverlay(dataType);
}

export function signatureScalarKind(dataType: ValueType): SignatureScalarKind {
  if (dataType.kind === "Array" || dataType.kind === "DataSeries") {
    return signatureScalarKind(dataType.inner);
  }
  if (dataType.kind === "Scalar") return dataType.inner;
  return dataType.kind === "DataFrame" ? "DataFrame" : "Object";
}

export function buildSignatureDataType(
  scalar: SignatureScalarKind,
  container?: SignatureContainerOverlay,
): ValueType {
  const base: ValueType =
    scalar === "Object" || scalar === "DataFrame"
      ? { kind: scalar }
      : { kind: "Scalar", inner: scalar };
  if (container === "array") return { kind: "Array", inner: base };
  if (container === "dataseries") return { kind: "DataSeries", inner: base };
  return base;
}

export function signatureEditorTypeOption(pin: { dataType: ValueType }): SignatureEditorTypeOption {
  return signatureScalarKind(pin.dataType);
}

export function applySignatureEditorType(
  pin: FunctionSignaturePin,
  option: SignatureEditorTypeOption,
): FunctionSignaturePin {
  const container = signatureContainerOverlay(pin.dataType);
  return {
    id: pin.id,
    name: pin.name,
    dataType: buildSignatureDataType(option, container),
  };
}

export function cycleSignatureContainer(pin: FunctionSignaturePin): FunctionSignaturePin {
  const scalar = signatureScalarKind(pin.dataType);
  const overlay = signatureContainerOverlay(pin.dataType);
  const next: SignatureContainerOverlay | undefined =
    overlay === "dataseries" ? undefined : "dataseries";
  return {
    id: pin.id,
    name: pin.name,
    dataType: buildSignatureDataType(scalar, next),
  };
}

export function createDataSignaturePin(
  id: string,
  name: string,
  dataType: ValueType,
): FunctionSignaturePin {
  return { id, name, dataType };
}

export function createDefaultDataSignaturePin(id: string, name: string): FunctionSignaturePin {
  return createDataSignaturePin(id, name, { kind: "Scalar", inner: "Numeric" });
}
