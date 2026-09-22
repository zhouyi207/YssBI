import type { ValueType } from "./valueType";

/** Rust 函数签名在编辑器中的投影。 */
export interface FunctionSignaturePin {
  id: string;
  name: string;
  dataType: ValueType;
}

export type FunctionPinSpec = FunctionSignaturePin;

export interface FunctionSignaturePatch {
  inputs?: FunctionPinSpec[];
  outputs?: FunctionPinSpec[];
}
