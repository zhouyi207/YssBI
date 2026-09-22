export type {
  ResultPlotKind as PlotChart,
  ResultPresentation as Presentation,
  ResultReportKind as ReportKind,
  ResultDescriptor,
  ResultPage,
  ResultValue,
} from "@/shared/types/domain/result";

export type ResultRendererKind = "sequence" | "scalar" | "json" | "plot" | "info";
