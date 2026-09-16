import { isBackendDataType, dataTypeFromKey, type ValueType } from "../domain/valueType";

export type DataTypeBackendFormat = ValueType;
export const isDataTypeBackendFormat = isBackendDataType;

export function dataTypeToBackend(type: ValueType): DataTypeBackendFormat {
  return type;
}

export function dataTypeFromBackend(value: unknown): ValueType {
  if (typeof value === "string") return dataTypeFromKey(value);
  if (isBackendDataType(value)) return value;
  throw new TypeError("Invalid analysis value type");
}
