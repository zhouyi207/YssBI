export type ChartType = "histogram" | "scatter" | "line";

export interface ChartEncodings {
  x?: string;
  y?: string;
}

export interface ChartDocumentState {
  databaseId: string;
  chartType: ChartType;
  encodings: ChartEncodings;
}

export interface ChartDocument extends ChartDocumentState {
  schemaVersion: number;
  revision: number;
}

export interface ChartIndexEntry {
  chartPath: string;
  name: string;
  databaseId: string;
  chartType: ChartType;
  revision: number;
}

export interface PlotColumnPairPayload {
  data: Array<{ x: number; y: number }>;
  xLabel?: string;
  yLabel?: string;
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
