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
      return {
        kind: "scatter",
        points: payload.data.data,
        referenceLines: payload.data.referenceLines,
        ...axes(payload.data),
      };
    case "line":
      return {
        kind: "line",
        points: payload.data.data,
        referenceLines: payload.data.referenceLines,
        ...axes(payload.data),
        showPoints: true,
      };
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
    case "boxplot":
    case "violin":
      return { kind: payload.kind, groups: payload.data.groups };
    case "wordcloud":
      return { kind: "wordcloud", words: payload.data.words };
    case "errorbar":
      return { kind: "errorbar", data: payload.data.data };
    case "coefficient":
      return { kind: "coefficient", data: payload.data.data };
    case "nomogram":
      return { kind: "nomogram", axes: payload.data.axes };
    case "pareto":
      return { kind: "pareto", data: payload.data.data };
    case "combination":
      return {
        kind: "combination",
        labels: payload.data.labels,
        bars: payload.data.bars,
        line: payload.data.line,
        dualAxis: payload.data.dualAxis,
      };
    case "heatmap":
      return {
        kind: "heatmap",
        xLabels: payload.data.xLabels,
        yLabels: payload.data.yLabels,
        matrix: payload.data.matrix,
      };
    case "bubble":
      return {
        kind: "scatter",
        points: payload.data.data,
        pointSizes: payload.data.data.map((point) => point.size),
        xAxis: { label: "X", valueType: "number" },
        yAxis: { label: "Y", valueType: "number" },
      };
    case "quadrant":
      return {
        kind: "scatter",
        points: payload.data.data,
        referenceLines: payload.data.referenceLines,
        ...axes(payload.data),
      };
    case "ppQq":
      return {
        kind: "scatter",
        points: payload.data.data,
        referenceLines: payload.data.referenceLines,
        ...axes(payload.data),
        ...(payload.data.mode === "pp"
          ? { xDomain: [0, 1] as [number, number], yDomain: [0, 1] as [number, number] }
          : {}),
      };
    case "roc":
      return {
        kind: "line",
        points: payload.data.data,
        referenceLines: payload.data.referenceLines,
        ...axes(payload.data),
        showPoints: false,
        xDomain: [0, 1],
        yDomain: [0, 1],
      };
  }
}
