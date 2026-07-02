/** Metadata-only source contract. Actual values live in backend sources. */
export type SourceKind = 'json' | 'dataframe' | 'series' | 'scalar' | 'null' | 'struct';

export type DataViewStructKind = 'ols_result' | 'unknown';

export type DataViewRendererKind =
  | 'dataframe'
  | 'series'
  | 'scalar'
  | 'null'
  | 'json'
  | 'plot'
  | 'info';

export interface SourceDescriptor {
  sourceId: string;
  kind: SourceKind;
  renderer: DataViewRendererKind;
  title: string;
  message?: string;
  executionTimeMs?: number;
  columns?: string[];
  totalRows?: number;
  name?: string;
  dtype?: string;
  length?: number;
  valueType?: string;
  typeKey?: string;
  handleId?: string;
  structKind?: DataViewStructKind;
}

export interface SourcePresentation {
  sourceId: string;
  route: '/view' | '/info' | '/plot' | string;
  windowTitle: string;
  plotType?: string;
}

export interface SourceValue {
  kind: SourceKind;
  title: string;
  message?: string;
  value?: unknown;
  valueType?: string;
  typeKey?: string;
  handleId?: string;
  structKind?: DataViewStructKind;
  structured?: unknown;
}

export interface SourcePage {
  kind: 'dataframe' | 'series';
  offset: number;
  limit: number;
  totalCount: number;
  columns?: string[];
  rows?: unknown[][];
  values?: unknown[];
}

export interface DataViewPageState {
  offset: number;
  limit: number;
  totalCount: number;
  rows: unknown[][];
  values: unknown[];
  loading: boolean;
  error: string | null;
}
