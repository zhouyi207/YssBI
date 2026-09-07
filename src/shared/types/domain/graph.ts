import type { DataType } from "./dataType";

export type GraphType = "event" | "function";

/** Rust 函数签名在编辑器中的投影。 */
export interface FunctionSignaturePin {
  id: string;
  name: string;
  dataType: DataType;
}

export type FunctionPinSpec = FunctionSignaturePin;

export interface FunctionSignaturePatch {
  inputs?: FunctionPinSpec[];
  outputs?: FunctionPinSpec[];
}
