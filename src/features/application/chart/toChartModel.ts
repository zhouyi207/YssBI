import type { ChartPreviewPayload } from "@/shared/types/domain";
import type { ChartModel } from "@/shared/charts/ChartModel";

export function toChartModel(payload: ChartPreviewPayload): ChartModel | null {
  switch (payload.kind) {
    case "histogram":
      return {
        kind: "histogram",
        bins: payload.bins,
        xLabel: payload.xLabel,
        yLabel: payload.yLabel,
      };
    case "scatter":
      return {
        kind: "scatter",
        points: payload.pair.data,
        xAxis: { label: payload.pair.xLabel, valueType: payload.pair.xFormat },
        yAxis: { label: payload.pair.yLabel, valueType: payload.pair.yFormat },
      };
    case "line":
      return {
        kind: "line",
        points: payload.pair.data,
        xAxis: { label: payload.pair.xLabel, valueType: payload.pair.xFormat },
        yAxis: { label: payload.pair.yLabel, valueType: payload.pair.yFormat },
        showPoints: true,
      };
    case "empty":
    case "error":
      return null;
  }
}
