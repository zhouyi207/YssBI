import type { GraphOutputRefDto } from "./executionDemand";

export type { GraphOutputRefDto } from "./executionDemand";

export type ResultId = string;

export const RESULT_PLOT_KINDS = [
  "scatter",
  "line",
  "plot",
  "ecdf",
  "kde",
  "histogram",
  "correlation",
  "correlogram",
] as const;

export type ResultPlotKind = (typeof RESULT_PLOT_KINDS)[number];

export function isResultPlotKind(value: unknown): value is ResultPlotKind {
  return typeof value === "string" && (RESULT_PLOT_KINDS as readonly string[]).includes(value);
}

export type ResultReportKind =
  | "olsSummary"
  | "binarySummary"
  | "iv2slsSummary"
  | "ivLimlSummary"
  | "praisSummary"
  | "varSummary"
  | "varSoc"
  | "panelSummary"
  | "panelDid"
  | "dfAdfSummary"
  | "dfAdfSummaryList"
  | "vecSummary"
  | "vecRankSummary";

export type ResultPresentation =
  | { kind: "inspector" }
  | { kind: "plot"; chart: ResultPlotKind }
  | { kind: "report"; report: ResultReportKind };

export interface ResultProvenance {
  runId: string;
  graphPath: string;
  nodeId: string;
  output: GraphOutputRefDto | null;
  createdAtMs: string;
}

export type ResultValueKind = "scalar" | "sequence" | "dataSeries" | "unknown";
export type DataSeriesElementType =
  | "int64"
  | "float64"
  | "string"
  | "boolean"
  | "date"
  | "datetime"
  | "categorical";

export interface ResultDataSeriesMetadata {
  elementType: DataSeriesElementType;
  length: number;
  nullCount: number;
  name: string | null;
  format: string | null;
}

export interface ResultDescriptor {
  resultId: ResultId;
  provenance: ResultProvenance;
  presentation: ResultPresentation;
  valueKind: ResultValueKind;
  metadata: ResultDataSeriesMetadata | null;
  totalCount: number | null;
  title: string;
}

export type ResultValue =
  | { kind: "value"; value: unknown }
  | { kind: "sequence"; value: unknown[] }
  | { kind: "dataSeries"; value: unknown[] };

export interface ResultPage {
  resultId: ResultId;
  offset: number;
  requestedLimit: number;
  actualCount: number;
  totalCount: number;
  hasMore: boolean;
  nextOffset: number | null;
  valueKind: Exclude<ResultValueKind, "unknown">;
  metadata: ResultDataSeriesMetadata | null;
  values: unknown[];
}
