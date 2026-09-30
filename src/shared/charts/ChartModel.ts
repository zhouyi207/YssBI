export type AxisValueType = "number" | "date" | "datetime";

export interface XYPoint {
  x: number;
  y: number;
}

export interface ReferenceLineModel {
  start: XYPoint;
  end: XYPoint;
}

export interface AxisModel {
  label?: string;
  valueType: AxisValueType;
}

export interface CorrelogramPoint {
  lag: number;
  value: number;
  qStat?: number;
  pValue?: number;
}

export interface HistogramBin {
  label: string;
  count: number;
}

export type ChartModel =
  | {
      kind: "scatter";
      points: XYPoint[];
      xAxis: AxisModel;
      yAxis: AxisModel;
      symmetricY?: boolean;
      zeroLine?: boolean;
      highlightIndices?: number[];
      referenceLines?: ReferenceLineModel[];
      pointSizes?: number[];
      xDomain?: [number, number];
      yDomain?: [number, number];
    }
  | {
      kind: "line";
      points: XYPoint[];
      xAxis: AxisModel;
      yAxis: AxisModel;
      showPoints: boolean;
      referenceLines?: ReferenceLineModel[];
      xDomain?: [number, number];
      yDomain?: [number, number];
    }
  | {
      kind: "histogram";
      bins: HistogramBin[];
      xLabel?: string;
      yLabel?: string;
      compact?: boolean;
    }
  | { kind: "ecdf"; points: XYPoint[]; xAxis: AxisModel; yAxis: AxisModel }
  | { kind: "kde"; points: XYPoint[]; xAxis: AxisModel; yAxis: AxisModel; xMin?: number }
  | {
      kind: "correlation";
      labels: string[];
      matrix: (number | null)[][];
      pMatrix?: (number | null)[][];
    }
  | {
      kind: "correlogram";
      acf: CorrelogramPoint[];
      pacf: CorrelogramPoint[];
      ciHalfWidth: number;
    }
  | { kind: "boxplot"; groups: DistributionGroupPlotDTO[] }
  | { kind: "violin"; groups: DistributionGroupPlotDTO[] }
  | { kind: "wordcloud"; words: WordCountPlotDTO[] }
  | { kind: "errorbar"; data: IntervalPointPlotDTO[] }
  | { kind: "coefficient"; data: CoefficientPointPlotDTO[] }
  | { kind: "pareto"; data: ParetoCategoryPlotDTO[] }
  | { kind: "combination"; labels: string[]; bars: number[]; line: number[]; dualAxis: boolean }
  | { kind: "heatmap"; xLabels: string[]; yLabels: string[]; matrix: number[][] };
import type {
  DistributionGroupPlotDTO,
  WordCountPlotDTO,
  IntervalPointPlotDTO,
  ParetoCategoryPlotDTO,
  CoefficientPointPlotDTO,
} from "@/shared/types/domain/plotPayload";
