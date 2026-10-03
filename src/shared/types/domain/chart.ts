export const CHART_TYPES = ["histogram", "scatter", "line"] as const;
export type ChartType = (typeof CHART_TYPES)[number];

export function isChartType(value: unknown): value is ChartType {
  return CHART_TYPES.some((chartType) => chartType === value);
}

export interface ChartEncodings {
  x?: string;
  y?: string;
}

export interface ChartDocumentState {
  databaseId: string;
  chartType: ChartType;
  encodings: ChartEncodings;
}

export function isChartDocumentState(value: unknown): value is ChartDocumentState {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const candidate = value as Record<string, unknown>;
  const keys = Object.keys(candidate);
  const encodings = candidate.encodings;
  return (
    keys.length === 3 &&
    keys.every((key) => key === "databaseId" || key === "chartType" || key === "encodings") &&
    typeof candidate.databaseId === "string" &&
    isChartType(candidate.chartType) &&
    encodings !== null &&
    typeof encodings === "object" &&
    !Array.isArray(encodings) &&
    Object.keys(encodings).every((key) => key === "x" || key === "y") &&
    Object.values(encodings).every((entry) => typeof entry === "string")
  );
}

export interface ChartDocument extends ChartDocumentState {
  schemaVersion: number;
}

export interface PlotColumnPairPayload {
  data: Array<{ x: number; y: number }>;
  xLabel: string | null;
  yLabel: string | null;
  xFormat: "date" | "datetime" | "number";
  yFormat: "date" | "datetime" | "number";
}

export type ChartPreviewPayload =
  | {
      kind: "histogram";
      bins: Array<{ label: string; count: number }>;
      xLabel?: string;
      yLabel?: string;
    }
  | { kind: "scatter" | "line"; pair: PlotColumnPairPayload }
  | { kind: "empty" }
  | {
      kind: "error";
      code: string;
      incidentId: string | null;
      column?: string;
    };
