/**
 * Plot 窗口 payload DTO（对齐 Rust `graph/register/catalog/plot/*` 序列化形态）
 * IPC → Plot 窗口边界单点窄化，禁止 View 层 `as unknown as`。
 */

import type { PlotChart } from '@/features/core/resultSource';
import {
  type PlotCorrelogramBarDTO,
  parsePlotCorrelogramBar,
} from '@/shared/types/report';
import type { AxisValueType } from '@/shared/types/visualization';

export interface PlotPointDTO {
  x: number;
  y: number;
}

export interface XySeriesPlotDTO {
  data: PlotPointDTO[];
  xLabel?: string;
  yLabel?: string;
  xFormat?: AxisValueType;
  yFormat?: AxisValueType;
}

export interface HistogramBinDTO {
  label: string;
  count: number;
}

export interface HistogramPlotDTO {
  data: HistogramBinDTO[];
  xLabel?: string;
  yLabel?: string;
}

export interface CorrelogramPlotDTO {
  acf: PlotCorrelogramBarDTO[];
  pacf: PlotCorrelogramBarDTO[];
  ciHalfWidth: number;
  n: number;
}

export interface CorrelationPlotDTO {
  labels: string[];
  matrix: (number | null)[][];
  pMatrix?: (number | null)[][];
}

export type ParsedPlotPayload =
  | { kind: 'correlogram'; data: CorrelogramPlotDTO }
  | { kind: 'histogram'; data: HistogramPlotDTO }
  | { kind: 'correlation'; data: CorrelationPlotDTO }
  | { kind: 'scatter'; data: XySeriesPlotDTO }
  | { kind: 'line'; data: XySeriesPlotDTO }
  | { kind: 'ecdf'; data: XySeriesPlotDTO }
  | { kind: 'kde'; data: XySeriesPlotDTO }
  | { kind: 'plot'; data: XySeriesPlotDTO };

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value);
}

function isNonNegativeInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isInteger(value) && value >= 0;
}

function readOptionalString(record: Record<string, unknown>, key: string): string | undefined {
  const value = record[key];
  if (typeof value === 'string' && value.length > 0) {
    return value;
  }
  return undefined;
}

function readOptionalAxisValueType(
  record: Record<string, unknown>,
  key: string,
): AxisValueType | undefined {
  const value = record[key];
  if (value === 'date' || value === 'datetime' || value === 'number') {
    return value;
  }
  return undefined;
}

function parsePlotPoint(raw: unknown): PlotPointDTO | null {
  if (!isRecord(raw)) return null;
  const x = raw.x;
  const y = raw.y;
  if (!isFiniteNumber(x) || !isFiniteNumber(y)) return null;
  return { x, y };
}

function parsePlotPointArray(raw: unknown): PlotPointDTO[] | null {
  if (!Array.isArray(raw) || raw.length === 0) return null;
  const points: PlotPointDTO[] = [];
  for (const item of raw) {
    const point = parsePlotPoint(item);
    if (!point) return null;
    points.push(point);
  }
  return points;
}

/** scatter / line / ecdf / kde 共用 XY 序列 payload */
export function parseXySeriesPlot(raw: unknown): XySeriesPlotDTO | null {
  if (!isRecord(raw)) return null;
  const data = parsePlotPointArray(raw.data);
  if (!data) return null;
  return {
    data,
    xLabel: readOptionalString(raw, 'xLabel'),
    yLabel: readOptionalString(raw, 'yLabel'),
    xFormat: readOptionalAxisValueType(raw, 'xFormat'),
    yFormat: readOptionalAxisValueType(raw, 'yFormat'),
  };
}

function parseHistogramBin(raw: unknown): HistogramBinDTO | null {
  if (!isRecord(raw)) return null;
  const label = raw.label;
  const count = raw.count;
  if (typeof label !== 'string' || !isNonNegativeInteger(count)) return null;
  return { label, count };
}

export function parseHistogramPlot(raw: unknown): HistogramPlotDTO | null {
  if (!isRecord(raw)) return null;
  if (!Array.isArray(raw.data) || raw.data.length === 0) return null;
  const bins: HistogramBinDTO[] = [];
  for (const item of raw.data) {
    const bin = parseHistogramBin(item);
    if (!bin) return null;
    bins.push(bin);
  }
  return {
    data: bins,
    xLabel: readOptionalString(raw, 'xLabel'),
    yLabel: readOptionalString(raw, 'yLabel'),
  };
}

function parseCorrelogramSeries(raw: unknown): PlotCorrelogramBarDTO[] | null {
  if (!Array.isArray(raw) || raw.length === 0) return null;
  const series: PlotCorrelogramBarDTO[] = [];
  for (const item of raw) {
    const datum = parsePlotCorrelogramBar(item);
    if (!datum) return null;
    series.push(datum);
  }
  return series;
}

export function parseCorrelogramPlot(raw: unknown): CorrelogramPlotDTO | null {
  if (!isRecord(raw)) return null;
  const acf = parseCorrelogramSeries(raw.acf);
  const pacf = parseCorrelogramSeries(raw.pacf);
  const ciHalfWidth = raw.ciHalfWidth;
  const n = raw.n;
  if (!acf || !pacf || !isFiniteNumber(ciHalfWidth) || !isNonNegativeInteger(n) || n === 0) {
    return null;
  }
  return { acf, pacf, ciHalfWidth, n };
}

function parseSquareNullableNumberMatrix(raw: unknown, size: number): (number | null)[][] | null {
  if (!Array.isArray(raw) || raw.length !== size) return null;
  const matrix: (number | null)[][] = [];
  for (const row of raw) {
    if (!Array.isArray(row) || row.length !== size) return null;
    const parsedRow: (number | null)[] = [];
    for (const cell of row) {
      if (cell !== null && !isFiniteNumber(cell)) return null;
      parsedRow.push(cell);
    }
    matrix.push(parsedRow);
  }
  return matrix;
}

export function parseCorrelationPlot(raw: unknown): CorrelationPlotDTO | null {
  if (!isRecord(raw)) return null;
  if (!Array.isArray(raw.labels) || raw.labels.length < 2) return null;
  const labels: string[] = [];
  for (const label of raw.labels) {
    if (typeof label !== 'string') return null;
    labels.push(label);
  }
  const matrix = parseSquareNullableNumberMatrix(raw.matrix, labels.length);
  if (!matrix) return null;
  const pMatrixRaw = raw.pMatrix;
  let pMatrix: (number | null)[][] | undefined;
  if (pMatrixRaw !== undefined) {
    const parsed = parseSquareNullableNumberMatrix(pMatrixRaw, labels.length);
    if (!parsed) return null;
    pMatrix = parsed;
  }
  return { labels, matrix, pMatrix };
}


/** 按 descriptor chart 窄化 plot payload；失败返回 null（由调用方渲染局部 invalid 状态）。 */
export function parsePlotPayload(chart: PlotChart, raw: unknown): ParsedPlotPayload | null {
  switch (chart) {
    case 'correlogram': {
      const data = parseCorrelogramPlot(raw);
      return data ? { kind: 'correlogram', data } : null;
    }
    case 'histogram': {
      const data = parseHistogramPlot(raw);
      return data ? { kind: 'histogram', data } : null;
    }
    case 'correlation': {
      const data = parseCorrelationPlot(raw);
      return data ? { kind: 'correlation', data } : null;
    }
    case 'scatter':
    case 'line':
    case 'plot':
    case 'ecdf':
    case 'kde': {
      const data = parseXySeriesPlot(raw);
      return data ? { kind: chart, data } : null;
    }
    default:
      return null;
  }
}
