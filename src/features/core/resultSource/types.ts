

export type {
  PlotChart,
  Presentation,
  ReportKind,
  SourceDescriptor,
  SourceKind,
  SourcePage,
  SourceStructKind,
  SourceValue,
} from '@/shared/types/dto/resultSource';

export type SourceRendererKind =
  | 'dataframe'
  | 'dataseries'
  | 'scalar'
  | 'null'
  | 'json'
  | 'plot'
  | 'info';

export interface SourcePageState {
  offset: number;
  limit: number;
  totalCount: number;
  rows: unknown[][];
  values: unknown[];
  loading: boolean;
  error: string | null;
}
