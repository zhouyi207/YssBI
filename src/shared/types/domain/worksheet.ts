export type WorksheetChartType = "histogram" | "scatter" | "line";

export interface WorksheetEncodings {
  x?: string;
  y?: string;
}

export interface WorksheetDocumentState {
  databaseId: string;
  chartType: WorksheetChartType;
  encodings: WorksheetEncodings;
}

export interface WorksheetDocument extends WorksheetDocumentState {
  schemaVersion: number;
  revision: number;
}

export interface WorksheetIndexEntry {
  worksheetPath: string;
  name: string;
  databaseId: string;
  chartType: WorksheetChartType;
  revision: number;
}

export interface PlotColumnPairPayload {
  data: Array<{ x: number; y: number }>;
  xLabel?: string;
  yLabel?: string;
  xFormat: "date" | "datetime" | "number";
  yFormat: "date" | "datetime" | "number";
}

export type WorksheetPreviewPayload =
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
