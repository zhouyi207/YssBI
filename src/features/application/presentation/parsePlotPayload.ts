import type { PlotChart } from '@/features/core/resultSource';

export interface ScatterEcdfData {
  data: { x: number; y: number }[];
  xLabel?: string;
  x_label?: string;
  yLabel?: string;
  y_label?: string;
  xFormat?: 'date' | 'datetime' | 'number';
  yFormat?: 'date' | 'datetime' | 'number';
}

export interface HistogramData {
  data: Array<{ label: string; count: number }>;
  x_label?: string;
  y_label?: string;
}

export interface CorrelationData {
  labels: string[];
  matrix: number[][];
  p_matrix?: number[][];
}

import type { CorrelogramDatum } from '@/views/PlotView/CorrelogramChart';

export interface CorrelogramData {
  acf: CorrelogramDatum[];
  pacf: CorrelogramDatum[];
  ci_half_width: number;
  n: number;
}

export type ParsedPlotPayload =
  | { kind: 'scatter_ecdf'; chart: PlotChart; data: ScatterEcdfData }
  | { kind: 'histogram'; data: HistogramData }
  | { kind: 'correlation'; data: CorrelationData }
  | { kind: 'correlogram'; data: CorrelogramData };

export function parsePlotPayload(chart: PlotChart, raw: unknown): ParsedPlotPayload | null {
  const parsed = raw as Record<string, unknown>;
  if (!parsed || typeof parsed !== 'object') return null;

  if (chart === 'correlogram') {
    if (Array.isArray(parsed.acf) && Array.isArray(parsed.pacf)) {
      return {
        kind: 'correlogram',
        data: parsed as unknown as CorrelogramData,
      };
    }
    return null;
  }

  if (chart === 'correlation') {
    if (Array.isArray(parsed.labels) && Array.isArray(parsed.matrix)) {
      return {
        kind: 'correlation',
        data: {
          labels: parsed.labels as string[],
          matrix: parsed.matrix as number[][],
          p_matrix: parsed.p_matrix as number[][] | undefined,
        },
      };
    }
    return null;
  }

  if (chart === 'histogram') {
    if (
      Array.isArray(parsed.data)
      && (parsed.data as unknown[]).every(
        (entry) =>
          entry
          && typeof entry === 'object'
          && typeof (entry as { label?: unknown }).label === 'string'
          && typeof (entry as { count?: unknown }).count === 'number',
      )
    ) {
      return {
        kind: 'histogram',
        data: parsed as unknown as HistogramData,
      };
    }
    return null;
  }

  if (Array.isArray(parsed.data)) {
    return {
      kind: 'scatter_ecdf',
      chart,
      data: parsed as unknown as ScatterEcdfData,
    };
  }

  return null;
}
