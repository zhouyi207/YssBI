export type AxisValueType = "number" | "date" | "datetime";

export interface XYPoint {
  x: number;
  y: number;
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
    };
