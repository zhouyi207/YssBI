import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { ChartService } from "./chartService";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const projectInstanceId = "00000000-0000-0000-0000-000000000601";
const operationId = "00000000-0000-0000-0000-000000000602";
const chartPath = "charts/Report.yssbi-chart";

function mutationResult() {
  return {
    operationId,
    projectInstanceId,
    publicationRevision: 1,
    moves: [],
    deltas: [],
    projectionReplacements: [],
    projectionStatus: { status: "complete", expectedGraphPaths: [] },
    history: { canUndo: false, canRedo: false },
  };
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("ChartService authoritative mutation contract", () => {
  it("invokes create with a required name and optional database ID", async () => {
    const result = mutationResult();
    vi.mocked(invoke).mockResolvedValue(result);

    await expect(
      ChartService.createChart(projectInstanceId, operationId, "Report", "database-1"),
    ).resolves.toEqual(result);

    expect(invoke).toHaveBeenCalledWith("create_chart", {
      projectInstanceId,
      operationId,
      name: "Report",
      databaseId: "database-1",
    });
  });

  it("invokes duplicate with canonical path and expected revision", async () => {
    vi.mocked(invoke).mockResolvedValue(mutationResult());

    await ChartService.duplicateChart(projectInstanceId, operationId, chartPath, 3);

    expect(invoke).toHaveBeenCalledWith("duplicate_chart", {
      projectInstanceId,
      operationId,
      chartPath,
      expectedRevision: 3,
    });
  });

  it("invokes save with path, expected revision, and path-free document", async () => {
    vi.mocked(invoke).mockResolvedValue(mutationResult());
    const document = {
      schemaVersion: 3,
      revision: 3,
      databaseId: "database-1",
      chartType: "line" as const,
      encodings: { x: "month", y: "sales" },
    };

    await ChartService.saveChart(projectInstanceId, operationId, chartPath, 3, document);

    expect(invoke).toHaveBeenCalledWith("save_chart", {
      projectInstanceId,
      operationId,
      chartPath,
      expectedRevision: 3,
      document,
    });
  });

  it("invokes rename with path, expected revision, exact name, and lifecycle token", async () => {
    vi.mocked(invoke).mockResolvedValue(mutationResult());

    await ChartService.renameChart(
      projectInstanceId,
      operationId,
      chartPath,
      3,
      "Renamed Report",
      7,
    );

    expect(invoke).toHaveBeenCalledWith("rename_chart_resource", {
      projectInstanceId,
      operationId,
      chartPath,
      expectedRevision: 3,
      newName: "Renamed Report",
      lifecycleToken: 7,
    });
  });

  it("invokes remove instead of the obsolete delete command", async () => {
    vi.mocked(invoke).mockResolvedValue(mutationResult());

    await ChartService.removeChart(projectInstanceId, operationId, chartPath, 3);

    expect(invoke).toHaveBeenCalledWith("remove_chart", {
      projectInstanceId,
      operationId,
      chartPath,
      expectedRevision: 3,
    });
  });

  it.each([
    ["create", () => ChartService.createChart(projectInstanceId, operationId, "Report")],
    ["duplicate", () => ChartService.duplicateChart(projectInstanceId, operationId, chartPath, 3)],
    [
      "save",
      () =>
        ChartService.saveChart(projectInstanceId, operationId, chartPath, 3, {
          schemaVersion: 3,
          revision: 3,
          databaseId: "",
          chartType: "histogram",
          encodings: {},
        }),
    ],
    [
      "rename",
      () =>
        ChartService.renameChart(projectInstanceId, operationId, chartPath, 3, "Renamed Report", 7),
    ],
    ["remove", () => ChartService.removeChart(projectInstanceId, operationId, chartPath, 3)],
  ])("strictly parses the %s mutation result", async (_label, request) => {
    vi.mocked(invoke).mockResolvedValue({ ...mutationResult(), unexpected: true });

    await expect(request()).rejects.toThrow("resource mutation result is malformed");
  });
});

describe("ChartService database read lifecycle contract", () => {
  it("passes exact project identity to plot column reads", async () => {
    vi.mocked(invoke).mockResolvedValue({
      data: [{ x: 1, y: 2 }],
      xLabel: "amount",
      yLabel: "cost",
      xFormat: "number",
      yFormat: "number",
    });

    await ChartService.getPlotColumnPair(projectInstanceId, "sales", "amount", "cost", 500);

    expect(invoke).toHaveBeenCalledWith("get_plot_column_pair", {
      projectInstanceId,
      databaseId: "sales",
      xCol: "amount",
      yCol: "cost",
      maxPoints: 500,
    });
  });
});
