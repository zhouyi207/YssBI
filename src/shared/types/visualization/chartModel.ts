import type { PlotCorrelogramBarDTO } from "@/shared/types/report";

export type AxisValueType = "number" | "date" | "datetime";

export interface XYPoint {
  x: number;
  y: number;
}

export interface AxisModel {
  label?: string;
  valueType: AxisValueType;
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
    }
  | {
      kind: "line";
      points: XYPoint[];
      xAxis: AxisModel;
      yAxis: AxisModel;
      showPoints: boolean;
    }
  | {
      kind: "histogram";
      bins: { label: string; count: number }[];
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
      acf: PlotCorrelogramBarDTO[];
      pacf: PlotCorrelogramBarDTO[];
      ciHalfWidth: number;
    };
