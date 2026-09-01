import { beforeEach, describe, expect, it, vi } from "vitest";

const ipc = vi.hoisted(() => ({
  response: undefined as unknown,
  invoke: vi.fn(async () => ipc.response),
}));

vi.mock("@tauri-apps/api/core", () => ({
  Channel: class {},
  invoke: ipc.invoke,
}));

import { ProjectService } from "./projectService";

function computationSettingsResult(
  overrides: Record<string, unknown> = {},
): Record<string, unknown> {
  return {
    projectInstanceId: "project-a",
    settingsRevision: 3,
    publicationRevision: 9,
    settings: {
      numeric: { tolerance: { absolute: 1e-12, relative: 1e-9 } },
      missingValues: { statistics: "listwise" },
    },
    ...overrides,
  };
}

function projectIndex(): Record<string, unknown> {
  return {
    projectInstanceId: "00000000-0000-0000-0000-000000000601",
    publicationRevision: 4,
    history: { canUndo: false, canRedo: false },
    projectName: "Projection contract",
    exportTime: "2026-08-07T00:00:00Z",
    graphs: [
      {
        path: "functions/forecast.yssbi-function",
        name: "Forecast",
        type: "function",
        revision: 11,
        functionRevision: 11,
        functionSignature: {
          parameters: [{ id: "sales", name: "Observed sales", type_name: "DataSeries<Float64>" }],
          return_type: "Array<String>",
        },
        functionEditorProjection: {
          functionRevision: 11,
          inputs: [
            {
              id: "sales",
              name: "Observed sales",
              dataType: { kind: "DataSeries", inner: { kind: "Float64" } },
            },
          ],
          outputs: [
            {
              id: "return",
              name: "Array<String>",
              dataType: { kind: "Array", inner: { kind: "String" } },
            },
          ],
        },
      },
    ],
    charts: [],
    variables: [],
    databases: [],
  };
}

function functionRow(index: Record<string, unknown>): Record<string, unknown> {
  return (index.graphs as Array<Record<string, unknown>>)[0];
}

function chartRow(): Record<string, unknown> {
  return {
    chartPath: "charts/Opaque Path With Spaces.yssbi-chart",
    name: "Rust supplied label",
    databaseId: "database-1",
    chartType: "scatter",
    revision: 7,
  };
}

describe("ProjectService computation settings", () => {
  beforeEach(() => {
    ipc.invoke.mockClear();
    ipc.response = computationSettingsResult();
  });

  it("strictly parses the backend snapshot and sends the current project identity", async () => {
    await expect(ProjectService.getProjectComputationSettings("project-a")).resolves.toEqual(
      computationSettingsResult(),
    );
    expect(ipc.invoke).toHaveBeenCalledWith("get_project_computation_settings", {
      projectInstanceId: "project-a",
    });
  });

  it.each([
    ["unknown top-level key", () => computationSettingsResult({ unexpected: true })],
    [
      "unsafe revision",
      () => computationSettingsResult({ settingsRevision: Number.MAX_SAFE_INTEGER + 1 }),
    ],
    [
      "unknown settings key",
      () => {
        const value = computationSettingsResult();
        (value.settings as Record<string, unknown>).unexpected = true;
        return value;
      },
    ],
    [
      "non-finite tolerance",
      () => {
        const value = computationSettingsResult();
        const numeric = (value.settings as Record<string, unknown>).numeric as Record<
          string,
          unknown
        >;
        (numeric.tolerance as Record<string, unknown>).absolute = Number.POSITIVE_INFINITY;
        return value;
      },
    ],
    [
      "both tolerances zero",
      () => {
        const value = computationSettingsResult();
        const numeric = (value.settings as Record<string, unknown>).numeric as Record<
          string,
          unknown
        >;
        numeric.tolerance = { absolute: 0, relative: 0 };
        return value;
      },
    ],
    [
      "unknown missing-value policy",
      () => {
        const value = computationSettingsResult();
        const missing = (value.settings as Record<string, unknown>).missingValues as Record<
          string,
          unknown
        >;
        missing.statistics = "pairwise";
        return value;
      },
    ],
  ])("rejects %s", async (_case, build) => {
    ipc.response = build();
    await expect(ProjectService.getProjectComputationSettings("project-a")).rejects.toThrow(
      "Invalid project computation settings response",
    );
  });

  it("strictly parses a correlated update receipt and sends one revisioned request", async () => {
    const settings = computationSettingsResult().settings as never;
    const receipt = computationSettingsResult({ operationId: "operation-a", settings });
    ipc.response = receipt;

    await expect(
      ProjectService.updateProjectComputationSettings({
        projectInstanceId: "project-a",
        operationId: "operation-a",
        expectedRevision: 3,
        settings,
      }),
    ).resolves.toEqual(receipt);
    expect(ipc.invoke).toHaveBeenCalledWith("update_project_computation_settings", {
      request: {
        projectInstanceId: "project-a",
        operationId: "operation-a",
        expectedRevision: 3,
        settings,
      },
    });
  });

  it("rejects an update receipt without the exact operation ID field", async () => {
    ipc.response = computationSettingsResult();
    await expect(
      ProjectService.updateProjectComputationSettings({
        projectInstanceId: "project-a",
        operationId: "operation-a",
        expectedRevision: 3,
        settings: computationSettingsResult().settings as never,
      }),
    ).rejects.toThrow("Invalid project computation settings receipt");
  });
});

describe("ProjectService save-as path contract", () => {
  beforeEach(() => {
    ipc.invoke.mockClear();
    ipc.response = { operationId: "operation-a", kind: "saveAs" };
  });

  it("forwards an application-selected destination without opening a native dialog", async () => {
    const destination = "C:/Projects/Selected Destination";

    await expect(
      ProjectService.saveProjectAs("project-a", "operation-a", destination),
    ).resolves.toEqual(ipc.response);

    expect(ipc.invoke).toHaveBeenNthCalledWith(1, "validate_new_project_path", {
      path: destination,
    });
    expect(ipc.invoke).toHaveBeenNthCalledWith(2, "save_project_as", {
      path: destination,
      projectInstanceId: "project-a",
      operationId: "operation-a",
    });
  });
});

describe("ProjectService.getProjectIndex function editor projection parser", () => {
  beforeEach(() => {
    ipc.invoke.mockClear();
    ipc.response = projectIndex();
  });

  it("preserves the exact Rust-resolved output name and structured pin types", async () => {
    const index = await ProjectService.getProjectIndex("00000000-0000-0000-0000-000000000601");

    const functionRow = index.graphs[0];
    expect(functionRow.type).toBe("function");
    if (functionRow.type !== "function") throw new Error("expected function row");
    expect(functionRow.functionEditorProjection).toEqual({
      functionRevision: 11,
      inputs: [
        {
          id: "sales",
          name: "Observed sales",
          dataType: { kind: "DataSeries", inner: { kind: "Float64" } },
        },
      ],
      outputs: [
        {
          id: "return",
          name: "Array<String>",
          dataType: { kind: "Array", inner: { kind: "String" } },
        },
      ],
    });
  });

  it("accepts opaque event and function paths containing spaces and Unicode", async () => {
    const index = projectIndex();
    const row = functionRow(index);
    row.path = "functions/Sales Report 销售预测.yssbi-function";
    (index.graphs as unknown[]).unshift({
      path: "events/每日 Sales Report.yssbi-event",
      name: "Daily report",
      type: "event",
      revision: 3,
    });
    ipc.response = index;

    await expect(ProjectService.getProjectIndex("project-a")).resolves.toMatchObject({
      graphs: [
        { path: "events/每日 Sales Report.yssbi-event", type: "event" },
        { path: "functions/Sales Report 销售预测.yssbi-function", type: "function" },
      ],
    });
  });

  it("rejects project rows whose type disagrees with the path kind or suffix", async () => {
    for (const path of [
      "events/Wrong.yssbi-event",
      "functions/Wrong.yssbi-event",
      "functions/Wrong.txt",
    ]) {
      const index = projectIndex();
      functionRow(index).path = path;
      ipc.response = index;
      await expect(ProjectService.getProjectIndex("project-a")).rejects.toThrow(
        "Invalid project index response",
      );
    }
  });

  it("rejects a function editor projection missing inputs", async () => {
    const index = projectIndex();
    const projection = functionRow(index).functionEditorProjection as Record<string, unknown>;
    delete projection.inputs;
    ipc.response = index;

    await expect(ProjectService.getProjectIndex("project-a")).rejects.toThrow(
      "Invalid project index response",
    );
  });

  it("rejects malformed structured data types instead of falling back to Any", async () => {
    const index = projectIndex();
    const projection = functionRow(index).functionEditorProjection as Record<string, unknown>;
    const input = (projection.inputs as Array<Record<string, unknown>>)[0];
    input.dataType = { kind: "DataSeries", inner: { kind: "UnsupportedInnerType" } };
    ipc.response = index;

    await expect(ProjectService.getProjectIndex("project-a")).rejects.toThrow(
      "Invalid project index response",
    );
  });

  it("rejects empty and whitespace-only Struct keys", async () => {
    for (const inner of ["", "   "]) {
      const index = projectIndex();
      const projection = functionRow(index).functionEditorProjection as Record<string, unknown>;
      const output = (projection.outputs as Array<Record<string, unknown>>)[0];
      output.dataType = { kind: "Struct", inner };
      ipc.response = index;

      await expect(ProjectService.getProjectIndex("project-a")).rejects.toThrow(
        "Invalid project index response",
      );
    }
  });

  it("requires every exact project-index key to be an own property", async () => {
    const index = projectIndex();
    const projectName = index.projectName;
    delete index.projectName;
    index.unknownProjectName = "substitution";
    ipc.response = Object.assign(Object.create({ projectName }), index);

    await expect(ProjectService.getProjectIndex("project-a")).rejects.toThrow(
      "Invalid project index response",
    );
  });

  it("strictly parses chart path identity and authoritative metadata", async () => {
    const index = projectIndex();
    index.charts = [chartRow()];
    ipc.response = index;

    await expect(ProjectService.getProjectIndex("project-a")).resolves.toMatchObject({
      charts: [
        {
          chartPath: "charts/Opaque Path With Spaces.yssbi-chart",
          name: "Rust supplied label",
          databaseId: "database-1",
          chartType: "scatter",
          revision: 7,
        },
      ],
    });
  });

  it.each([
    [
      "empty Rust-provided name",
      (row: Record<string, unknown>) => {
        row.name = "";
      },
    ],
    [
      "whitespace-only Rust-provided name",
      (row: Record<string, unknown>) => {
        row.name = "   ";
      },
    ],
    [
      "obsolete id",
      (row: Record<string, unknown>) => {
        row.id = row.chartPath;
        delete row.chartPath;
      },
    ],
    [
      "missing revision",
      (row: Record<string, unknown>) => {
        delete row.revision;
      },
    ],
    [
      "unknown field",
      (row: Record<string, unknown>) => {
        row.unexpectedName = "inferred";
      },
    ],
    [
      "unsupported chart type",
      (row: Record<string, unknown>) => {
        row.chartType = "pie";
      },
    ],
  ])("rejects chart rows with %s", async (_case, mutate) => {
    const index = projectIndex();
    const row = chartRow();
    mutate(row);
    index.charts = [row];
    ipc.response = index;

    await expect(ProjectService.getProjectIndex("project-a")).rejects.toThrow(
      "Invalid project index response",
    );
  });

  it("requires functionEditorProjection on function rows", async () => {
    const index = projectIndex();
    delete functionRow(index).functionEditorProjection;
    ipc.response = index;

    await expect(ProjectService.getProjectIndex("project-a")).rejects.toThrow(
      "Invalid project index response",
    );
  });
});
