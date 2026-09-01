import type { ParsedPlotPayload, XySeriesPlotDTO } from "@/shared/types/dto/plotPayload";
import type { AxisModel, ChartModel } from "@/shared/charts/ChartModel";

function axes(data: XySeriesPlotDTO): { xAxis: AxisModel; yAxis: AxisModel } {
  return {
    xAxis: { label: data.xLabel, valueType: data.xFormat ?? "number" },
    yAxis: { label: data.yLabel, valueType: data.yFormat ?? "number" },
  };
}

export function toResultChartModel(payload: ParsedPlotPayload): ChartModel {
  switch (payload.kind) {
    case "scatter":
    case "plot":
      return { kind: "scatter", points: payload.data.data, ...axes(payload.data) };
    case "line":
      return { kind: "line", points: payload.data.data, ...axes(payload.data), showPoints: true };
    case "histogram":
      return {
        kind: "histogram",
        bins: payload.data.data,
        xLabel: payload.data.xLabel,
        yLabel: payload.data.yLabel,
      };
    case "ecdf":
      return { kind: "ecdf", points: payload.data.data, ...axes(payload.data) };
    case "kde":
      return { kind: "kde", points: payload.data.data, ...axes(payload.data) };
    case "correlation":
      return {
        kind: "correlation",
        labels: payload.data.labels,
        matrix: payload.data.matrix,
        pMatrix: payload.data.pMatrix,
      };
    case "correlogram":
      return {
        kind: "correlogram",
        acf: payload.data.acf,
        pacf: payload.data.pacf,
        ciHalfWidth: payload.data.ciHalfWidth,
      };
  }
}
