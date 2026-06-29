/** Metadata-only DataView contract. Actual values live in backend sources. */
export type DataViewDataType = 'dataframe' | 'series' | 'scalar' | 'null' | 'struct';

export type DataViewStructKind = 'ols_result' | 'unknown';

export interface WindowSourceMetadata {
  sourceId: string;
  windowType: string;
  viewType?: 'data_view' | 'window_source';
  dataType?: string;
  renderer?: string;
  title: string;
  message?: string;
  executionTimeMs?: number;
}

export interface DataViewPayload extends WindowSourceMetadata {
  viewType: 'data_view';
  dataType: DataViewDataType;
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

export type DataViewRendererKind =
  | 'dataframe'
  | 'series'
  | 'scalar'
  | 'null'
  | 'struct_ols'
  | 'struct_generic';

export interface DataViewSourceValue {
  viewType: 'data_view';
  dataType: DataViewDataType;
  title: string;
  message?: string;
  value?: unknown;
  valueType?: string;
  typeKey?: string;
  handleId?: string;
  structKind?: DataViewStructKind;
  structured?: unknown;
}

export interface WindowDataPageResponse {
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
