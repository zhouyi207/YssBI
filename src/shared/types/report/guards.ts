/** IPC / JSON 边界窄化共用守卫 */

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

export function isFiniteNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

export function isNonNegativeInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isInteger(value) && value >= 0;
}

export function isString(value: unknown): value is string {
  return typeof value === "string";
}

export function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every(isString);
}

/** 已校验 record 上的可选 string 字段 */
export function optionalString(raw: Record<string, unknown>, key: string): string | undefined {
  return isString(raw[key]) ? raw[key] : undefined;
}

/** 已校验 record 上的可选有限 number 字段 */
export function optionalFiniteNumber(
  raw: Record<string, unknown>,
  key: string,
): number | undefined {
  return isFiniteNumber(raw[key]) ? raw[key] : undefined;
}

/** 已校验 record 上的可选非负整数 字段 */
export function optionalNonNegativeInteger(
  raw: Record<string, unknown>,
  key: string,
): number | undefined {
  return isNonNegativeInteger(raw[key]) ? raw[key] : undefined;
}

/**
 * 将 raw 中已存在的可选键浅拷贝到 base（IPC 边界保留未逐字段窄化的嵌套块）。
 * 调用方须已校验 base 的必填字段。
 */
export function assignPresentKeys<T extends object>(
  base: T,
  raw: Record<string, unknown>,
  keys: readonly string[],
): T {
  const out = { ...base };
  for (const key of keys) {
    if (raw[key] !== undefined) {
      Object.assign(out, { [key]: raw[key] });
    }
  }
  return out;
}
