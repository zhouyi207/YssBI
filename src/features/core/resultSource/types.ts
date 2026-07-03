/** Metadata-only source contract. Actual values live in backend sources. */
export type SourceKind = 'json' | 'dataframe' | 'dataseries' | 'scalar' | 'null' | 'struct';

export type SourceStructKind = 'ols_result' | 'unknown';

export type PlotChart =
  | 'scatter'
  | 'line'
  | 'plot'
  | 'ecdf'
  | 'kde'
  | 'histogram'
  | 'correlation'
  | 'correlogram';

export type ReportKind =
  | 'olsSummary'
  | 'varSummary'
  | 'varSoc'
  | 'panelSummary'
  | 'panelDid'
  | 'dfAdfSummary'
  | 'dfAdfSummaryList'
  | 'vecSummary'
  | 'vecRankSummary';

export type Presentation =
  | { kind: 'inspector' }
  | { kind: 'plot'; chart: PlotChart }
  | { kind: 'report'; report: ReportKind };

export type SourceRendererKind =
  | 'dataframe'
  | 'dataseries'
  | 'scalar'
  | 'null'
  | 'json'
  | 'plot'
  | 'info';

export interface SourceDescriptor {
  sourceId: string;
  kind: SourceKind;
  presentation: Presentation;
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
  structKind?: SourceStructKind;
}

export interface SourceValue {
  kind: SourceKind;
  title: string;
  message?: string;
  value?: unknown;
  valueType?: string;
  typeKey?: string;
  handleId?: string;
  structKind?: SourceStructKind;
  structured?: unknown;
}

export interface SourcePage {
  kind: 'dataframe' | 'dataseries';
  offset: number;
  limit: number;
  totalCount: number;
  columns?: string[];
  rows?: unknown[][];
  values?: unknown[];
}

export interface SourcePageState {
  offset: number;
  limit: number;
  totalCount: number;
  rows: unknown[][];
  values: unknown[];
  loading: boolean;
  error: string | null;
}
