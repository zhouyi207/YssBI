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

/** Plot 窗口 correlogram 柱条（Rust 必填 q_stat / p_value） */
export interface PlotCorrelogramBarDTO extends CorrelogramBarDTO {
  q_stat: number;
  p_value: number;
}

export function hasLjungBoxStats(bar: CorrelogramBarDTO): bar is PlotCorrelogramBarDTO {
  const candidate = bar as PlotCorrelogramBarDTO;
  return isFiniteNumber(candidate.q_stat) && isFiniteNumber(candidate.p_value);
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
  const q_stat = raw.q_stat;
  const p_value = raw.p_value;
  if (
    !isNonNegativeInteger(lag)
    || !isFiniteNumber(value)
    || !isFiniteNumber(q_stat)
    || !isFiniteNumber(p_value)
  ) {
    return null;
  }
  return { lag, value, q_stat, p_value };
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
    `Q(${bar.lag}): <b>${bar.q_stat.toFixed(4)}</b><br/>` +
    `p-value: <b>${formatPValueDisplay(bar.p_value)}</b>`
  );
}
