import { isBackendDataType, dataTypeFromKey, type ValueType } from "../domain/valueType";

export function dataTypeFromBackend(value: unknown): ValueType {
  if (typeof value === "string") return dataTypeFromKey(value);
  if (isBackendDataType(value)) return value;
  throw new TypeError("Invalid analysis value type");
}
