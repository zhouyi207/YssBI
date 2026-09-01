import { beforeEach, describe, expect, it, vi } from "vitest";
import { DatabaseService } from "@/services/database/databaseService";
import { normalizeIpcError } from "@/services/ipc";
import { ChartService } from "@/services/chart/chartService";
import type { ChartType, ChartDocument } from "@/shared/types/domain";
import { fetchChartPreview } from "./chartPreviewDataService";

const projectInstanceId = "00000000-0000-0000-0000-000000000611";

function document(chartType: ChartType): ChartDocument {
  return {
    schemaVersion: 3,
    revision: 0,
    databaseId: "sales",
    chartType,
    encodings: chartType === "histogram" ? { x: "amount" } : { x: "amount", y: "cost" },
  };
}

function identity() {
  return {
    projectInstanceId,
    isCurrent: () => true,
    assertCurrent: vi.fn(),
  };
}

describe("fetchChartPreview machine errors", () => {
  beforeEach(() => vi.restoreAllMocks());

  it("returns a machine error with only the safe missing column context", async () => {
    vi.spyOn(DatabaseService, "getColumnDistribution").mockResolvedValue([
      { columnName: "cost", kind: "numeric", bins: [] },
    ]);

    const result = await fetchChartPreview(document("histogram"), identity());

    expect(result).toEqual({
      kind: "error",
      code: "chart_preview_column_not_found",
      incidentId: null,
      column: "amount",
    });
    expect(result).not.toHaveProperty("message");
  });

  it.each(["histogram", "scatter", "line"] as const)(
    "maps a %s parser failure to a stable fallback without raw prose",
    async (chartType) => {
      const rawParserText = `private ${chartType} parser failure`;
      if (chartType === "histogram") {
        vi.spyOn(DatabaseService, "getColumnDistribution").mockRejectedValue(
          new Error(rawParserText),
        );
      } else {
        vi.spyOn(ChartService, "getPlotColumnPair").mockRejectedValue(new Error(rawParserText));
      }

      const result = await fetchChartPreview(document(chartType), identity());

      expect(result).toEqual({
        kind: "error",
        code: "chart_preview_read_failed",
        incidentId: null,
      });
      expect(JSON.stringify(result)).not.toContain(rawParserText);
      expect(result).not.toHaveProperty("message");
    },
  );

  it("preserves IPC code and incident ID while dropping backend prose", async () => {
    vi.spyOn(ChartService, "getPlotColumnPair").mockRejectedValue(
      normalizeIpcError("get_plot_column_pair", {
        code: "chart_plot_pair_unavailable",
        details: { detail: "private backend detail" },
        incidentId: "incident-chart-42",
      }),
    );

    const result = await fetchChartPreview(document("line"), identity());

    expect(result).toEqual({
      kind: "error",
      code: "chart_plot_pair_unavailable",
      incidentId: "incident-chart-42",
    });
    expect(JSON.stringify(result)).not.toContain("private backend detail");
    expect(result).not.toHaveProperty("message");
  });
});
