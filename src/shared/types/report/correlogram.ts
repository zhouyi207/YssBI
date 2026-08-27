/**
 * Correlogram 柱条 DTO
 * - Report（Info ACF/PACF）：仅 lag + value
 * - Plot（Rust Correlogram 节点）：含 Ljung-Box Q / p-value
 */

import { isFiniteNumber, isNonNegativeInteger, isRecord } from './guards';

/** Info 报告 / 残差 ACF·PACF 柱条（无 Q 统计量） */
export interface CorrelogramBarDTO {
  lag: number;
  value: number;
}

/** Plot 窗口 correlogram 柱条（Rust 必填 qStat / pValue） */
export interface PlotCorrelogramBarDTO extends CorrelogramBarDTO {
  qStat: number;
  pValue: number;
}

export function hasLjungBoxStats(bar: CorrelogramBarDTO): bar is PlotCorrelogramBarDTO {
  const candidate = bar as PlotCorrelogramBarDTO;
  return isFiniteNumber(candidate.qStat) && isFiniteNumber(candidate.pValue);
}

export function parseCorrelogramBar(raw: unknown): CorrelogramBarDTO | null {
  if (!isRecord(raw)) return null;
  const lag = raw.lag;
  const value = raw.value;
  if (!isNonNegativeInteger(lag) || !isFiniteNumber(value)) return null;
  return { lag, value };
}

export function parsePlotCorrelogramBar(raw: unknown): PlotCorrelogramBarDTO | null {
  if (!isRecord(raw)) return null;
  const lag = raw.lag;
  const value = raw.value;
  const qStat = raw.qStat;
  const pValue = raw.pValue;
  if (
    !isNonNegativeInteger(lag)
    || !isFiniteNumber(value)
    || !isFiniteNumber(qStat)
    || !isFiniteNumber(pValue)
  ) {
    return null;
  }
  return { lag, value, qStat, pValue };
}

export function acfSeriesToBars(acf: number[]): CorrelogramBarDTO[] {
  return acf.map((value, i) => ({ lag: i, value }));
}

export function pacfSeriesToBars(pacf: number[]): CorrelogramBarDTO[] {
  return pacf.map((value, i) => ({ lag: i + 1, value }));
}

export function formatPValueDisplay(p: number): string {
  return p < 0.0001 ? p.toExponential(2) : p.toFixed(4);
}

/** Correlogram tooltip 中 Q / p-value 行（仅 Plot 柱条有内容） */
export function correlogramLjungBoxTooltipHtml(bar: CorrelogramBarDTO): string {
  if (!hasLjungBoxStats(bar)) return '';
  return (
    `Q(${bar.lag}): <b>${bar.qStat.toFixed(4)}</b><br/>` +
    `p-value: <b>${formatPValueDisplay(bar.pValue)}</b>`
  );
}
