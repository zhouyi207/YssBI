import { describe, expect, it } from "vitest";
import type { PlotColumnPairPayload, ChartPreviewPayload } from "@/shared/types/domain";
import { toChartModel } from "./toChartModel";

describe("toChartModel", () => {
  it("maps a Chart line preview to line data-space semantics", () => {
    const pair: PlotColumnPairPayload = {
      data: [{ x: 2, y: 8 }],
      xLabel: "Date",
      yLabel: "Revenue",
      xFormat: "date",
      yFormat: "number",
    };

    expect(toChartModel({ kind: "line", pair })).toMatchObject({
      kind: "line",
      points: pair.data,
      xAxis: { label: "Date", valueType: "date" },
      yAxis: { label: "Revenue", valueType: "number" },
      showPoints: true,
    });
  });

  it.each<ChartPreviewPayload>([
    { kind: "empty" },
    { kind: "error", code: "chart_preview_failed", incidentId: "incident-1" },
  ])("leaves the $kind preview state to the Chart view", (payload) => {
    expect(toChartModel(payload)).toBeNull();
  });
});
