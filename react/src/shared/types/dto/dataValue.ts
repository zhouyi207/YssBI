import type { SerializedDataValue } from "../domain/dataValue";

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isInteger(value: unknown, signed: boolean): value is string {
  if (typeof value !== "string" || !/^(?:0|-?[1-9]\d*)$/.test(value)) return false;
  const integer = BigInt(value);
  return signed
    ? integer >= -(1n << 63n) && integer < 1n << 63n
    : integer >= 0n && integer < 1n << 64n;
}

/** The same literal contract is used throughout Graph and Node protocol. */
export function isRustDataValueWire(value: unknown): value is SerializedDataValue {
  if (value === "Null") return true;
  if (!isRecord(value) || Object.keys(value).length !== 1) return false;
  if ("Bool" in value) return typeof value.Bool === "boolean";
  if ("Integer" in value) return isInteger(value.Integer, true);
  if ("Unsigned" in value) return isInteger(value.Unsigned, false);
  if ("Decimal" in value)
    return (
      typeof value.Decimal === "string" &&
      /^-?(?:0|[1-9]\d*)(?:\.\d*[1-9])?$/.test(value.Decimal) &&
      value.Decimal !== "-0"
    );
  if ("String" in value) return typeof value.String === "string";
  if ("Bytes" in value)
    return (
      Array.isArray(value.Bytes) &&
      value.Bytes.every((byte) => Number.isInteger(byte) && byte >= 0 && byte <= 255)
    );
  if ("List" in value) return Array.isArray(value.List) && value.List.every(isRustDataValueWire);
  return (
    "Object" in value &&
    isRecord(value.Object) &&
    Object.values(value.Object).every(isRustDataValueWire)
  );
}
