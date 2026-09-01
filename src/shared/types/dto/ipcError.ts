export interface IpcErrorDto {
  code: string;
  details: Record<string, unknown> | null;
  incidentId: string | null;
}

const LOWER_SNAKE_CASE_PATTERN = /^[a-z][a-z0-9]*(?:_[a-z0-9]+)*$/;
const IPC_ERROR_KEYS = ["code", "details", "incidentId"] as const;

function isRecord(value: unknown): value is Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

function hasExactKeys(value: Record<string, unknown>, keys: readonly string[]): boolean {
  const actual = Object.keys(value);
  return (
    actual.length === keys.length &&
    keys.every((key) => Object.prototype.hasOwnProperty.call(value, key))
  );
}

export function isIpcErrorDto(value: unknown): value is IpcErrorDto {
  return (
    isRecord(value) &&
    hasExactKeys(value, IPC_ERROR_KEYS) &&
    typeof value.code === "string" &&
    LOWER_SNAKE_CASE_PATTERN.test(value.code) &&
    (value.details === null || isRecord(value.details)) &&
    (value.incidentId === null || typeof value.incidentId === "string")
  );
}

export function parseIpcErrorDto(value: unknown): IpcErrorDto {
  if (!isIpcErrorDto(value)) throw new Error("Invalid IPC error response");
  return {
    code: value.code,
    details: value.details,
    incidentId: value.incidentId,
  };
}
