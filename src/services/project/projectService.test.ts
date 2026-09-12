import { projectIndexSnapshotFixture } from "@/tests/helpers/activityPanelFixture";
import type { ProjectIndexRow } from "@/shared/types/domain/project";
import { beforeEach, describe, expect, it, vi } from "vitest";

const ipc = vi.hoisted(() => ({
  response: undefined as unknown,
  invoke: vi.fn(async (command: string) => {
    if (command !== "get_project_index") return ipc.response;
    const snapshot = projectIndexSnapshotFixture(ipc.response as ProjectIndexRow);
    return {
      index: ipc.response,
      activityPanels: Object.fromEntries(
        Object.entries(snapshot.activityPanels).map(([id, value]) => [
          id,
          { kind: "snapshot", ...value },
        ]),
      ),
    };
  }),
}));

vi.mock("@tauri-apps/api/core", () => ({
  Channel: class {},
  invoke: ipc.invoke,
}));

import { ProjectService } from "./projectService";

function projectIndex(): Record<string, unknown> {
  return {
    projectInstanceId: "project-a",
    publicationRevision: 4,
    projectName: "Projection contract",
    exportTime: "2026-08-07T00:00:00",
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

describe("ProjectService.getProjectIndex function editor projection parser", () => {
  beforeEach(() => {
    ipc.invoke.mockClear();
    ipc.response = projectIndex();
  });

  it("preserves the exact Rust-resolved output name and structured pin types", async () => {
    const { index } = await ProjectService.getProjectIndex("project-a");

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
      index: {
        graphs: [
          { path: "events/每日 Sales Report.yssbi-event", type: "event" },
          { path: "functions/Sales Report 销售预测.yssbi-function", type: "function" },
        ],
      },
    });
  });

  it("uses the explicit graph type while preserving an opaque path", async () => {
    const index = projectIndex();
    functionRow(index).path = "events/opaque-function-identity";
    ipc.response = index;
    await expect(ProjectService.getProjectIndex("project-a")).resolves.toMatchObject({
      index: {
        graphs: [{ path: "events/opaque-function-identity", type: "function" }],
      },
    });
    functionRow(index).path = "";
    await expect(ProjectService.getProjectIndex("project-a")).rejects.toThrow(
      "Invalid project index response",
    );
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
      index: {
        charts: [
          {
            chartPath: "charts/Opaque Path With Spaces.yssbi-chart",
            name: "Rust supplied label",
            databaseId: "database-1",
            chartType: "scatter",
            revision: 7,
          },
        ],
      },
    });
    index.charts = [chartRow(), chartRow()];
    await expect(ProjectService.getProjectIndex("project-a")).rejects.toThrow(
      "Invalid project index response",
    );
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
