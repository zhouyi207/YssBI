/**
 * Plot payload DTO aligned with Rust `yss-sci-contract::visualization`.
 * IPC → Plot 窗口边界单点窄化，禁止 View 层 `as unknown as`。
 */

import type { ResultPlotKind as PlotChart } from "@/shared/types/domain/result";
import { type PlotCorrelogramBarDTO, parsePlotCorrelogramBar } from "@/shared/types/report";
import { z } from "zod";

const numericDomainSchema = z
  .tuple([z.number().finite(), z.number().finite()])
  .refine(([start, end]) => start !== end)
  .optional();

const nomogramSchema = z.object({
  axes: z
    .array(
      z.object({
        label: z.string(),
        ticks: z
          .array(z.object({ position: z.number().finite().min(0).max(1), label: z.string() }))
          .min(1),
      }),
    )
    .min(4),
  horizon: z.number().finite().positive(),
  maximumTotalPoints: z.number().finite().positive(),
  pointsPerLogHazard: z.number().finite().positive(),
  baselineCumulativeHazard: z.number().finite().positive(),
});
export type NomogramPlotDTO = z.infer<typeof nomogramSchema>;

export type AxisFormat = "date" | "datetime" | "number";

export interface PlotPointDTO {
  x: number;
  y: number;
}

export interface PlotMetadataDTO {
  observations: number;
  displayed: number;
  sampled: boolean;
}

export interface PlotReferenceLineDTO {
  start: PlotPointDTO;
  end: PlotPointDTO;
}

export interface XySeriesPlotDTO {
  data: PlotPointDTO[];
  xLabel?: string;
  yLabel?: string;
  xFormat?: AxisFormat;
  yFormat?: AxisFormat;
  referenceLines?: PlotReferenceLineDTO[];
  metadata?: PlotMetadataDTO;
  yDomain?: [number, number];
}

export interface HistogramBinDTO {
  label: string;
  count: number;
}

export interface HistogramPlotDTO {
  data: HistogramBinDTO[];
  xLabel?: string;
  yLabel?: string;
  observations?: number;
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

export interface DistributionGroupPlotDTO {
  label: string;
  observations: number;
  lowerWhisker: number;
  q1: number;
  median: number;
  q3: number;
  upperWhisker: number;
  outliers: number[];
  outlierCount: number;
  density: PlotPointDTO[];
}

export interface DistributionPlotDTO {
  groups: DistributionGroupPlotDTO[];
}
export interface WordCountPlotDTO {
  label: string;
  count: number;
}
export interface WordCloudPlotDTO {
  words: WordCountPlotDTO[];
  observations: number;
  uniqueWords: number;
}
export interface IntervalPointPlotDTO extends PlotPointDTO {
  lower: number;
  upper: number;
}
export interface IntervalPlotDTO {
  data: IntervalPointPlotDTO[];
  metadata?: PlotMetadataDTO;
}
export interface ProbabilityPlotDTO extends XySeriesPlotDTO {
  mode: "pp" | "qq";
  referenceMean: number;
  referenceStandardDeviation: number;
}
export interface RocPlotDTO extends XySeriesPlotDTO {
  auc: number;
  positives: number;
  negatives: number;
}
export interface QuadrantPlotDTO extends XySeriesPlotDTO {
  xCut: number;
  yCut: number;
  counts: [number, number, number, number];
}
export interface ParetoCategoryPlotDTO extends WordCountPlotDTO {
  cumulative: number;
}
export interface ParetoPlotDTO {
  data: ParetoCategoryPlotDTO[];
  observations: number;
}
export interface CombinationPlotDTO {
  labels: string[];
  bars: number[];
  line: number[];
  dualAxis: boolean;
  metadata?: PlotMetadataDTO;
}
export interface BubblePointPlotDTO extends PlotPointDTO {
  size: number;
}
export interface BubblePlotDTO {
  data: BubblePointPlotDTO[];
  metadata?: PlotMetadataDTO;
}
export interface HeatmapPlotDTO {
  xLabels: string[];
  yLabels: string[];
  matrix: number[][];
  metadata?: PlotMetadataDTO;
}
export interface CoefficientPointPlotDTO {
  label: string;
  value: number;
  lower: number;
  upper: number;
}
export interface CoefficientPlotDTO {
  data: CoefficientPointPlotDTO[];
  confidenceLevel: number;
}

export type ParsedPlotPayload =
  | { kind: "correlogram"; data: CorrelogramPlotDTO }
  | { kind: "histogram"; data: HistogramPlotDTO }
  | { kind: "correlation"; data: CorrelationPlotDTO }
  | { kind: "scatter"; data: XySeriesPlotDTO }
  | { kind: "line"; data: XySeriesPlotDTO }
  | { kind: "ecdf"; data: XySeriesPlotDTO }
  | { kind: "kde"; data: XySeriesPlotDTO }
  | { kind: "boxplot"; data: DistributionPlotDTO }
  | { kind: "violin"; data: DistributionPlotDTO }
  | { kind: "wordcloud"; data: WordCloudPlotDTO }
  | { kind: "errorbar"; data: IntervalPlotDTO }
  | { kind: "ppQq"; data: ProbabilityPlotDTO }
  | { kind: "roc"; data: RocPlotDTO }
  | { kind: "quadrant"; data: QuadrantPlotDTO }
  | { kind: "pareto"; data: ParetoPlotDTO }
  | { kind: "combination"; data: CombinationPlotDTO }
  | { kind: "bubble"; data: BubblePlotDTO }
  | { kind: "heatmap"; data: HeatmapPlotDTO }
  | { kind: "coefficient"; data: CoefficientPlotDTO }
  | { kind: "nomogram"; data: NomogramPlotDTO };

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

function isNonNegativeInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
}

function isPositiveInteger(value: unknown): value is number {
  return isNonNegativeInteger(value) && value > 0;
}

function parseRows<T>(raw: unknown, parse: (value: unknown) => T | null, minimum = 1): T[] | null {
  if (!Array.isArray(raw) || raw.length < minimum) return null;
  const result: T[] = [];
  for (const value of raw) {
    const parsed = parse(value);
    if (parsed === null) return null;
    result.push(parsed);
  }
  return result;
}

function parseMetadata(raw: unknown): PlotMetadataDTO | undefined | null {
  if (raw === undefined) return undefined;
  if (
    !isRecord(raw) ||
    !isPositiveInteger(raw.observations) ||
    !isPositiveInteger(raw.displayed) ||
    typeof raw.sampled !== "boolean"
  )
    return null;
  return { observations: raw.observations, displayed: raw.displayed, sampled: raw.sampled };
}

function parseReferenceLine(raw: unknown): PlotReferenceLineDTO | null {
  if (!isRecord(raw)) return null;
  const start = parsePlotPoint(raw.start);
  const end = parsePlotPoint(raw.end);
  return start && end ? { start, end } : null;
}

function parseNumbers(raw: unknown, minimum = 1): number[] | null {
  return parseRows(raw, (value) => (isFiniteNumber(value) ? value : null), minimum);
}

function parseStrings(raw: unknown): string[] | null {
  return parseRows(raw, (value) => (typeof value === "string" ? value : null));
}

function readOptionalString(record: Record<string, unknown>, key: string): string | undefined {
  const value = record[key];
  if (typeof value === "string" && value.length > 0) {
    return value;
  }
  return undefined;
}

function readOptionalAxisFormat(
  record: Record<string, unknown>,
  key: string,
): AxisFormat | undefined {
  const value = record[key];
  if (value === "date" || value === "datetime" || value === "number") {
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
  const referenceLines =
    raw.referenceLines === undefined
      ? undefined
      : parseRows(raw.referenceLines, parseReferenceLine, 0);
  const metadata = parseMetadata(raw.metadata);
  const yDomain = numericDomainSchema.safeParse(raw.yDomain);
  if (referenceLines === null || metadata === null || !yDomain.success) return null;
  return {
    data,
    xLabel: readOptionalString(raw, "xLabel"),
    yLabel: readOptionalString(raw, "yLabel"),
    xFormat: readOptionalAxisFormat(raw, "xFormat"),
    yFormat: readOptionalAxisFormat(raw, "yFormat"),
    referenceLines,
    metadata,
    ...(yDomain.data ? { yDomain: yDomain.data } : {}),
  };
}

function parseDistribution(raw: unknown, violin: boolean): DistributionPlotDTO | null {
  if (!isRecord(raw)) return null;
  const groups = parseRows(raw.groups, (value) => {
    if (
      !isRecord(value) ||
      typeof value.label !== "string" ||
      !isPositiveInteger(value.observations) ||
      !isNonNegativeInteger(value.outlierCount)
    )
      return null;
    const { lowerWhisker, q1, median, q3, upperWhisker } = value;
    if (
      !isFiniteNumber(lowerWhisker) ||
      !isFiniteNumber(q1) ||
      !isFiniteNumber(median) ||
      !isFiniteNumber(q3) ||
      !isFiniteNumber(upperWhisker) ||
      lowerWhisker > q1 ||
      q1 > median ||
      median > q3 ||
      q3 > upperWhisker
    )
      return null;
    const outliers = parseNumbers(value.outliers, 0);
    const density = parseRows(value.density, parsePlotPoint, violin ? 2 : 0);
    if (
      !outliers ||
      !density ||
      value.outlierCount < outliers.length ||
      density.some((p) => p.y < 0)
    )
      return null;
    return {
      label: value.label,
      observations: value.observations,
      lowerWhisker,
      q1,
      median,
      q3,
      upperWhisker,
      outliers,
      outlierCount: value.outlierCount,
      density,
    };
  });
  return groups ? { groups } : null;
}

function parseWordCount(raw: unknown): WordCountPlotDTO | null {
  return isRecord(raw) &&
    typeof raw.label === "string" &&
    raw.label.trim().length > 0 &&
    isPositiveInteger(raw.count)
    ? { label: raw.label, count: raw.count }
    : null;
}

function parseWordCloud(raw: unknown): WordCloudPlotDTO | null {
  if (!isRecord(raw) || !isPositiveInteger(raw.observations) || !isPositiveInteger(raw.uniqueWords))
    return null;
  const words = parseRows(raw.words, parseWordCount);
  if (
    !words ||
    raw.uniqueWords < words.length ||
    words.reduce((sum, value) => sum + value.count, 0) > raw.observations
  )
    return null;
  return { words, observations: raw.observations, uniqueWords: raw.uniqueWords };
}

function parseIntervalPoint(raw: unknown): IntervalPointPlotDTO | null {
  const point = parsePlotPoint(raw);
  if (
    !point ||
    !isRecord(raw) ||
    !isFiniteNumber(raw.lower) ||
    !isFiniteNumber(raw.upper) ||
    raw.lower > point.y ||
    raw.upper < point.y
  )
    return null;
  return { ...point, lower: raw.lower, upper: raw.upper };
}

function parseIntervals(raw: unknown): IntervalPlotDTO | null {
  if (!isRecord(raw)) return null;
  const data = parseRows(raw.data, parseIntervalPoint);
  const metadata = parseMetadata(raw.metadata);
  return data && metadata !== null ? { data, metadata } : null;
}

function parseProbability(raw: unknown): ProbabilityPlotDTO | null {
  const plot = parseXySeriesPlot(raw);
  if (
    !plot ||
    !isRecord(raw) ||
    (raw.mode !== "pp" && raw.mode !== "qq") ||
    !isFiniteNumber(raw.referenceMean) ||
    !isFiniteNumber(raw.referenceStandardDeviation) ||
    raw.referenceStandardDeviation <= 0
  )
    return null;
  return {
    ...plot,
    mode: raw.mode,
    referenceMean: raw.referenceMean,
    referenceStandardDeviation: raw.referenceStandardDeviation,
  };
}

function parseRoc(raw: unknown): RocPlotDTO | null {
  const plot = parseXySeriesPlot(raw);
  if (
    !plot ||
    !isRecord(raw) ||
    !isFiniteNumber(raw.auc) ||
    raw.auc < 0 ||
    raw.auc > 1 ||
    !isPositiveInteger(raw.positives) ||
    !isPositiveInteger(raw.negatives)
  )
    return null;
  if (
    plot.data.some(
      (point, i) =>
        point.x < 0 ||
        point.x > 1 ||
        point.y < 0 ||
        point.y > 1 ||
        (i > 0 && (point.x < plot.data[i - 1].x || point.y < plot.data[i - 1].y)),
    )
  )
    return null;
  const first = plot.data[0];
  const last = plot.data[plot.data.length - 1];
  if (first.x !== 0 || first.y !== 0 || last?.x !== 1 || last.y !== 1) return null;
  return { ...plot, auc: raw.auc, positives: raw.positives, negatives: raw.negatives };
}

function parseQuadrant(raw: unknown): QuadrantPlotDTO | null {
  const plot = parseXySeriesPlot(raw);
  if (
    !plot ||
    !isRecord(raw) ||
    !isFiniteNumber(raw.xCut) ||
    !isFiniteNumber(raw.yCut) ||
    !Array.isArray(raw.counts) ||
    raw.counts.length !== 4 ||
    !raw.counts.every(isNonNegativeInteger)
  )
    return null;
  return {
    ...plot,
    xCut: raw.xCut,
    yCut: raw.yCut,
    counts: [raw.counts[0], raw.counts[1], raw.counts[2], raw.counts[3]],
  };
}

function parsePareto(raw: unknown): ParetoPlotDTO | null {
  if (!isRecord(raw) || !isPositiveInteger(raw.observations)) return null;
  const data = parseRows(raw.data, (value) => {
    const counted = parseWordCount(value);
    return counted &&
      isRecord(value) &&
      isFiniteNumber(value.cumulative) &&
      value.cumulative > 0 &&
      value.cumulative <= 1
      ? { ...counted, cumulative: value.cumulative }
      : null;
  });
  if (
    !data ||
    data[data.length - 1]?.cumulative !== 1 ||
    data.reduce((sum, row) => sum + row.count, 0) !== raw.observations ||
    data.some((row, i) => i > 0 && row.cumulative < data[i - 1].cumulative)
  )
    return null;
  return { data, observations: raw.observations };
}

function parseCombination(raw: unknown): CombinationPlotDTO | null {
  if (!isRecord(raw) || typeof raw.dualAxis !== "boolean") return null;
  const labels = parseStrings(raw.labels);
  const bars = parseNumbers(raw.bars);
  const line = parseNumbers(raw.line);
  const metadata = parseMetadata(raw.metadata);
  return labels &&
    bars &&
    line &&
    labels.length === bars.length &&
    bars.length === line.length &&
    metadata !== null
    ? { labels, bars, line, dualAxis: raw.dualAxis, metadata }
    : null;
}

function parseBubble(raw: unknown): BubblePlotDTO | null {
  if (!isRecord(raw)) return null;
  const data = parseRows(raw.data, (value) => {
    const point = parsePlotPoint(value);
    return point && isRecord(value) && isFiniteNumber(value.size) && value.size >= 0
      ? { ...point, size: value.size }
      : null;
  });
  const metadata = parseMetadata(raw.metadata);
  return data && metadata !== null ? { data, metadata } : null;
}

function parseHeatmap(raw: unknown): HeatmapPlotDTO | null {
  if (!isRecord(raw)) return null;
  const xLabels = parseStrings(raw.xLabels);
  const yLabels = parseStrings(raw.yLabels);
  const matrix = parseRows(raw.matrix, (value) => parseNumbers(value));
  const metadata = parseMetadata(raw.metadata);
  return xLabels &&
    yLabels &&
    matrix &&
    metadata !== null &&
    matrix.length === yLabels.length &&
    matrix.every((row) => row.length === xLabels.length)
    ? { xLabels, yLabels, matrix, metadata }
    : null;
}

function parseCoefficients(raw: unknown): CoefficientPlotDTO | null {
  if (
    !isRecord(raw) ||
    !isFiniteNumber(raw.confidenceLevel) ||
    raw.confidenceLevel <= 0 ||
    raw.confidenceLevel >= 1
  )
    return null;
  const data = parseRows(raw.data, (value) => {
    if (
      !isRecord(value) ||
      typeof value.label !== "string" ||
      !isFiniteNumber(value.value) ||
      !isFiniteNumber(value.lower) ||
      !isFiniteNumber(value.upper) ||
      value.lower > value.value ||
      value.upper < value.value
    )
      return null;
    return { label: value.label, value: value.value, lower: value.lower, upper: value.upper };
  });
  return data ? { data, confidenceLevel: raw.confidenceLevel } : null;
}

function parseHistogramBin(raw: unknown): HistogramBinDTO | null {
  if (!isRecord(raw)) return null;
  const label = raw.label;
  const count = raw.count;
  if (typeof label !== "string" || !isNonNegativeInteger(count)) return null;
  return { label, count };
}

export function parseHistogramPlot(raw: unknown): HistogramPlotDTO | null {
  if (!isRecord(raw)) return null;
  if (raw.observations !== undefined && !isPositiveInteger(raw.observations)) return null;
  if (!Array.isArray(raw.data) || raw.data.length === 0) return null;
  const bins: HistogramBinDTO[] = [];
  for (const item of raw.data) {
    const bin = parseHistogramBin(item);
    if (!bin) return null;
    bins.push(bin);
  }
  return {
    data: bins,
    xLabel: readOptionalString(raw, "xLabel"),
    yLabel: readOptionalString(raw, "yLabel"),
    observations: isPositiveInteger(raw.observations) ? raw.observations : undefined,
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
  if (
    !acf ||
    !pacf ||
    !isFiniteNumber(ciHalfWidth) ||
    ciHalfWidth <= 0 ||
    !isNonNegativeInteger(n) ||
    n === 0
  ) {
    return null;
  }
  if (acf.some((bar) => bar.qStat === undefined || bar.pValue === undefined)) return null;
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
    if (typeof label !== "string") return null;
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
    case "nomogram": {
      const parsed = nomogramSchema.safeParse(raw);
      return parsed.success ? { kind: chart, data: parsed.data } : null;
    }
    case "boxplot":
    case "violin": {
      const data = parseDistribution(raw, chart === "violin");
      return data ? { kind: chart, data } : null;
    }
    case "wordcloud": {
      const data = parseWordCloud(raw);
      return data ? { kind: chart, data } : null;
    }
    case "errorbar": {
      const data = parseIntervals(raw);
      return data ? { kind: chart, data } : null;
    }
    case "ppQq": {
      const data = parseProbability(raw);
      return data ? { kind: chart, data } : null;
    }
    case "roc": {
      const data = parseRoc(raw);
      return data ? { kind: chart, data } : null;
    }
    case "quadrant": {
      const data = parseQuadrant(raw);
      return data ? { kind: chart, data } : null;
    }
    case "pareto": {
      const data = parsePareto(raw);
      return data ? { kind: chart, data } : null;
    }
    case "combination": {
      const data = parseCombination(raw);
      return data ? { kind: chart, data } : null;
    }
    case "bubble": {
      const data = parseBubble(raw);
      return data ? { kind: chart, data } : null;
    }
    case "heatmap": {
      const data = parseHeatmap(raw);
      return data ? { kind: chart, data } : null;
    }
    case "coefficient": {
      const data = parseCoefficients(raw);
      return data ? { kind: chart, data } : null;
    }
    case "correlogram": {
      const data = parseCorrelogramPlot(raw);
      return data ? { kind: "correlogram", data } : null;
    }
    case "histogram": {
      const data = parseHistogramPlot(raw);
      return data ? { kind: "histogram", data } : null;
    }
    case "correlation": {
      const data = parseCorrelationPlot(raw);
      return data ? { kind: "correlation", data } : null;
    }
    case "scatter":
    case "line":
    case "ecdf":
    case "kde": {
      const data = parseXySeriesPlot(raw);
      return data ? { kind: chart, data } : null;
    }
    default:
      return null;
  }
}
