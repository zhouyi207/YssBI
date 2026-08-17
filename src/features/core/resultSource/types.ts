import type { ErrorReference } from '@/services/ipc';

export type {
  ResultPlotKind as PlotChart,
  ResultPresentation as Presentation,
  ResultReportKind as ReportKind,
  ResultDescriptor,
  ResultPage,
  ResultValue,
} from '@/shared/types/dto/result';

export type ResultRendererKind = 'sequence' | 'dataseries' | 'scalar' | 'json' | 'plot' | 'info';

export interface ResultPageState {
  offset: number;
  limit: number;
  totalCount: number;
  rows: unknown[][];
  values: unknown[];
  loading: boolean;
  error: ErrorReference | null;
}
