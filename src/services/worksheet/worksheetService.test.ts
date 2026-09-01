import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { WorksheetService } from "./worksheetService";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const projectInstanceId = "00000000-0000-0000-0000-000000000601";
const operationId = "00000000-0000-0000-0000-000000000602";
const worksheetPath = "worksheets/Report.yssbi-worksheet";

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

describe("WorksheetService authoritative mutation contract", () => {
  it("invokes create with a required name and optional database ID", async () => {
    const result = mutationResult();
    vi.mocked(invoke).mockResolvedValue(result);

    await expect(
      WorksheetService.createWorksheet(projectInstanceId, operationId, "Report", "database-1"),
    ).resolves.toEqual(result);

    expect(invoke).toHaveBeenCalledWith("create_worksheet", {
      projectInstanceId,
      operationId,
      name: "Report",
      databaseId: "database-1",
    });
  });

  it("invokes duplicate with canonical path and expected revision", async () => {
    vi.mocked(invoke).mockResolvedValue(mutationResult());

    await WorksheetService.duplicateWorksheet(projectInstanceId, operationId, worksheetPath, 3);

    expect(invoke).toHaveBeenCalledWith("duplicate_worksheet", {
      projectInstanceId,
      operationId,
      worksheetPath,
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

    await WorksheetService.saveWorksheet(
      projectInstanceId,
      operationId,
      worksheetPath,
      3,
      document,
    );

    expect(invoke).toHaveBeenCalledWith("save_worksheet", {
      projectInstanceId,
      operationId,
      worksheetPath,
      expectedRevision: 3,
      document,
    });
  });

  it("invokes rename with path, expected revision, exact name, and lifecycle token", async () => {
    vi.mocked(invoke).mockResolvedValue(mutationResult());

    await WorksheetService.renameWorksheet(
      projectInstanceId,
      operationId,
      worksheetPath,
      3,
      "Renamed Report",
      7,
    );

    expect(invoke).toHaveBeenCalledWith("rename_worksheet_resource", {
      projectInstanceId,
      operationId,
      worksheetPath,
      expectedRevision: 3,
      newName: "Renamed Report",
      lifecycleToken: 7,
    });
  });

  it("invokes remove instead of the obsolete delete command", async () => {
    vi.mocked(invoke).mockResolvedValue(mutationResult());

    await WorksheetService.removeWorksheet(projectInstanceId, operationId, worksheetPath, 3);

    expect(invoke).toHaveBeenCalledWith("remove_worksheet", {
      projectInstanceId,
      operationId,
      worksheetPath,
      expectedRevision: 3,
    });
  });

  it.each([
    ["create", () => WorksheetService.createWorksheet(projectInstanceId, operationId, "Report")],
    [
      "duplicate",
      () => WorksheetService.duplicateWorksheet(projectInstanceId, operationId, worksheetPath, 3),
    ],
    [
      "save",
      () =>
        WorksheetService.saveWorksheet(projectInstanceId, operationId, worksheetPath, 3, {
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
        WorksheetService.renameWorksheet(
          projectInstanceId,
          operationId,
          worksheetPath,
          3,
          "Renamed Report",
          7,
        ),
    ],
    [
      "remove",
      () => WorksheetService.removeWorksheet(projectInstanceId, operationId, worksheetPath, 3),
    ],
  ])("strictly parses the %s mutation result", async (_label, request) => {
    vi.mocked(invoke).mockResolvedValue({ ...mutationResult(), unexpected: true });

    await expect(request()).rejects.toThrow("resource mutation result is malformed");
  });
});

describe("WorksheetService database read lifecycle contract", () => {
  it("passes exact project identity to plot column reads", async () => {
    vi.mocked(invoke).mockResolvedValue({
      data: [{ x: 1, y: 2 }],
      xLabel: "amount",
      yLabel: "cost",
      xFormat: "number",
      yFormat: "number",
    });

    await WorksheetService.getPlotColumnPair(projectInstanceId, "sales", "amount", "cost", 500);

    expect(invoke).toHaveBeenCalledWith("get_plot_column_pair", {
      projectInstanceId,
      databaseId: "sales",
      xCol: "amount",
      yCol: "cost",
      maxPoints: 500,
    });
  });
});
