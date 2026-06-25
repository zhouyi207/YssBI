export type WorksheetChartType = 'histogram' | 'scatter' | 'line';

export interface WorksheetEncodings {
  x?: string;
  y?: string;
}

export interface WorksheetDocument {
  id: string;
  name: string;
  databaseId: string;
  chartType: WorksheetChartType;
  encodings: WorksheetEncodings;
  folderPath?: string;
}

export interface WorksheetIndexEntry {
  id: string;
  name: string;
  databaseId: string;
  chartType: WorksheetChartType;
  folderPath: string;
}

export interface PlotColumnPairPayload {
  data: Array<{ x: number; y: number }>;
  xLabel?: string;
  yLabel?: string;
  xFormat: 'date' | 'datetime' | 'number';
  yFormat: 'date' | 'datetime' | 'number';
}

export type WorksheetPreviewPayload =
  | { kind: 'histogram'; bins: Array<{ label: string; count: number }>; xLabel?: string; yLabel?: string }
  | { kind: 'scatter' | 'line'; pair: PlotColumnPairPayload }
  | { kind: 'empty' }
  | { kind: 'error'; message: string };
