/**
 * Correlogram 柱条 DTO
 * - Report（Info ACF/PACF）：仅 lag + value
 * - Plot（Rust Correlogram 节点）：含 Ljung-Box Q / p-value
 */

import { isFiniteNumber, isNonNegativeInteger, isRecord } from "./guards";

/** Info 报告 / 残差 ACF·PACF 柱条（无 Q 统计量） */
export interface CorrelogramBarDTO {
  lag: number;
  value: number;
}

/** ACF carries Ljung–Box statistics; PACF has no corresponding Q test. */
export interface PlotCorrelogramBarDTO extends CorrelogramBarDTO {
  qStat?: number;
  pValue?: number;
}

export function parsePlotCorrelogramBar(raw: unknown): PlotCorrelogramBarDTO | null {
  if (!isRecord(raw)) return null;
  const lag = raw.lag;
  const value = raw.value;
  const qStat = raw.qStat;
  const pValue = raw.pValue;
  if (
    !isNonNegativeInteger(lag) ||
    !isFiniteNumber(value) ||
    (qStat !== undefined && qStat !== null && (!isFiniteNumber(qStat) || qStat < 0)) ||
    (pValue !== undefined &&
      pValue !== null &&
      (!isFiniteNumber(pValue) || pValue < 0 || pValue > 1))
  ) {
    return null;
  }
  return {
    lag,
    value,
    qStat: isFiniteNumber(qStat) ? qStat : undefined,
    pValue: isFiniteNumber(pValue) ? pValue : undefined,
  };
}

export function acfSeriesToBars(acf: readonly number[]): CorrelogramBarDTO[] {
  return acf.map((value, i) => ({ lag: i, value }));
}

export function pacfSeriesToBars(pacf: readonly number[]): CorrelogramBarDTO[] {
  return pacf.map((value, i) => ({ lag: i + 1, value }));
}
