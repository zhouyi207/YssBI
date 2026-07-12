/**
 * InfoView 统计数值展示 — 单点防御，避免裸 `.toFixed()` 在漂移 JSON 上崩溃。
 */

const FALLBACK = '—';

/** 将未知值窄化为有限 number；拒绝对象/数组等非标量。 */
export function coerceFiniteNumber(value: unknown): number | null {
  if (typeof value === 'number') {
    return Number.isFinite(value) ? value : null;
  }
  if (typeof value === 'string') {
    const trimmed = value.trim();
    if (!trimmed) return null;
    const n = Number(trimmed);
    return Number.isFinite(n) ? n : null;
  }
  return null;
}

export function formatNum(value: unknown, decimals = 4): string {
  if (typeof value === 'number' && !Number.isFinite(value)) {
    return 'Inf';
  }
  const n = coerceFiniteNumber(value);
  if (n === null) return FALLBACK;
  if (Math.abs(n) >= 1e6) return n.toExponential(2);
  if (Math.abs(n) < 0.0001 && n !== 0) return n.toExponential(3);
  return n.toFixed(decimals);
}

export function formatNullableNum(
  value: unknown,
  decimals = 4,
  fallback: string = FALLBACK,
): string {
  if (value === null || value === undefined) return fallback;
  const formatted = formatNum(value, decimals);
  return formatted === FALLBACK ? fallback : formatted;
}

export function formatPercent(value: unknown, decimals = 2): string {
  const n = coerceFiniteNumber(value);
  if (n === null) return FALLBACK;
  return `${(n * 100).toFixed(decimals)}%`;
}
