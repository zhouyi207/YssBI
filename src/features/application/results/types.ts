import type { ErrorReference } from "@/shared/types/domain/diagnostics";

export type {
  ResultPlotKind as PlotChart,
  ResultPresentation as Presentation,
  PinResultEntry,
  ResultReportKind as ReportKind,
  ResultDescriptor,
  ResultPage,
  ResultValue,
} from "@/shared/types/domain/result";

export type ResultRendererKind = "sequence" | "dataseries" | "scalar" | "json" | "plot" | "info";

export interface ResultPageState {
  offset: number;
  limit: number;
  totalCount: number;
  rows: unknown[][];
  values: unknown[];
  loading: boolean;
  error: ErrorReference | null;
}
