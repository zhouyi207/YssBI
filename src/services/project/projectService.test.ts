import { projectIndexSnapshotFixture } from "@/tests/helpers/activityPanelFixture";
import registeredProject from "@/tests/fixtures/project-event-wire/project-record.json";
import type { ProjectIndexRow } from "@/shared/types/domain/project";
import { beforeEach, describe, expect, it, vi } from "vitest";

const ipc = vi.hoisted(() => ({
  response: undefined as unknown,
  invoke: vi.fn(async (command: string, _args?: Record<string, unknown>) => {
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
import { logger } from "@/utils/frontendLogger";

vi.mock("@/utils/frontendLogger", () => ({ logger: { sys: { warn: vi.fn() } } }));

it("validates activation, registry and lifecycle command replies using current project contracts", async () => {
  const activation = {
    path: registeredProject.path,
    projectInstanceId: "project-a",
    activationRevision: 7,
  };
  ipc.response = { ...activation, activationRevision: -1 };
  await expect.soft(ProjectService.getProjectActivation()).rejects.toThrow();
  ipc.response = activation;
  await expect(ProjectService.loadProjectToState(registeredProject.path)).resolves.toEqual(
    activation,
  );
  ipc.response = [{ ...registeredProject, isFavorite: "false" }];
  await expect.soft(ProjectService.listRegisteredProjects()).rejects.toThrow();
  ipc.response = [registeredProject];
  await expect.soft(ProjectService.listRegisteredProjects()).resolves.toEqual([registeredProject]);
  ipc.response = registeredProject;
  await expect
    .soft(ProjectService.registerProject(registeredProject.name, registeredProject.path))
    .resolves.toEqual(registeredProject);
  ipc.response = { discovered: 1, newlyRegistered: 2, projects: [registeredProject] };
  await expect.soft(ProjectService.scanProjectsInDirectory("D:/Projects")).rejects.toThrow();
  ipc.response = { discovered: 1, newlyRegistered: 1, projects: [registeredProject] };
  await expect
    .soft(ProjectService.scanProjectsInDirectory("D:/Projects"))
    .resolves.toEqual(ipc.response);
  ipc.response = { removed: 0 };
  await expect
    .soft(ProjectService.cleanupInvalidRegisteredProjects())
    .resolves.toEqual(ipc.response);
  ipc.response = [registeredProject];
  await expect.soft(ProjectService.listRegisteredProjects()).resolves.toEqual([registeredProject]);
  const lifecycle = {
    operationId: "operation-a",
    kind: "create",
    oldProjectInstanceId: null,
    newProjectInstanceId: null,
    phase: "registryCommitted",
    outcome: "activationFailed",
    record: registeredProject,
    path: registeredProject.path,
    recovery: null,
    invalidation: { project: false, registry: true },
  };
  ipc.response = { ...lifecycle, invalidation: { project: "false", registry: true } };
  await expect
    .soft(ProjectService.createProject("Project", registeredProject.path, lifecycle.operationId))
    .rejects.toThrow();
  ipc.response = lifecycle;
  await expect
    .soft(ProjectService.createProject("Project", registeredProject.path, lifecycle.operationId))
    .resolves.toEqual(lifecycle);
  ipc.response = { ...lifecycle, kind: "saveAs" };
  await expect
    .soft(ProjectService.saveProjectAs("project-a", lifecycle.operationId, registeredProject.path))
    .resolves.toEqual(ipc.response);
  ipc.response = {
    ...lifecycle,
    kind: "delete",
    oldProjectInstanceId: activation.projectInstanceId,
    phase: "authorityCommitted",
    outcome: "committed",
    invalidation: { project: true, registry: true },
  };
  await expect
    .soft(
      ProjectService.deleteRegisteredProjectFiles(
        registeredProject.id,
        activation.projectInstanceId,
        lifecycle.operationId,
      ),
    )
    .resolves.toEqual(ipc.response);
  expect.soft(ipc.invoke).toHaveBeenLastCalledWith("delete_registered_project_files", {
    id: registeredProject.id,
    expectedActiveInstanceId: activation.projectInstanceId,
    operationId: lifecycle.operationId,
  });
  for (const rootIdentityState of ["valid", "invalid"]) {
    const record = { ...registeredProject, rootIdentity: "", rootIdentityState };
    ipc.response = [record];
    await expect.soft(ProjectService.listRegisteredProjects()).resolves.toEqual([record]);
  }
  const missingState = { ...registeredProject } as Partial<typeof registeredProject>;
  delete missingState.rootIdentityState;
  for (const record of [
    missingState,
    { ...registeredProject, rootIdentityState: "unknown" },
    { ...registeredProject, legacyIdentity: "unexpected" },
  ]) {
    ipc.response = [record];
    await expect
      .soft(ProjectService.listRegisteredProjects())
      .rejects.toThrow("Invalid project record");
  }
  ipc.response = "false";
  await expect
    .soft(ProjectService.toggleRegisteredProjectFavorite(registeredProject.id))
    .rejects.toThrow();
});

it("ignores malformed picker progress and stops delivery when its command ends", async () => {
  let channel!: { onmessage(value: unknown): void };
  const scanProgress = vi.fn();
  ipc.invoke.mockImplementationOnce(async (_command, args) => {
    channel = args!.onProgress as typeof channel;
    channel.onmessage({ kind: "registering", current: 2, total: 1 });
    channel.onmessage({ kind: "discovered", count: 1 });
    return { discovered: 1, newlyRegistered: 1, projects: [registeredProject] };
  });
  await ProjectService.scanProjectsInDirectory("D:/Projects", scanProgress);
  channel.onmessage({ kind: "registering", current: 1, total: 1 });
  expect.soft(scanProgress.mock.calls).toEqual([[{ kind: "discovered", count: 1 }]]);
  const cleanupProgress = vi.fn();
  ipc.invoke.mockImplementationOnce(async (_command, args) => {
    channel = args!.onProgress as typeof channel;
    channel.onmessage({ kind: "removing", removed: -1, total: 1 });
    channel.onmessage({ kind: "checking", current: 1, total: 1 });
    return { removed: 0 };
  });
  await ProjectService.cleanupInvalidRegisteredProjects(cleanupProgress);
  channel.onmessage({ kind: "removing", removed: 1, total: 1 });
  expect.soft(cleanupProgress.mock.calls).toEqual([[{ kind: "checking", current: 1, total: 1 }]]);
  expect(logger.sys.warn).toHaveBeenCalledTimes(2);
});

function projectIndex(): Record<string, unknown> {
  return {
    projectInstanceId: "project-a",
    publicationRevision: 4,
    projectName: "Projection contract",
    exportTime: "2026-08-07T00:00:00",
    eventGraphs: [],
    functionGraphs: [
      {
        path: "functions/forecast.yssbi-function",
        name: "Forecast",
        type: "function_graph",
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
              dataType: { kind: "DataSeries", inner: { kind: "Scalar", inner: "Numeric" } },
            },
          ],
          outputs: [
            {
              id: "return",
              name: "Array<String>",
              dataType: { kind: "Array", inner: { kind: "Scalar", inner: "Text" } },
            },
          ],
        },
      },
    ],
    minds: [],
    docs: [],
    charts: [],
    databases: [],
  };
}

function functionRow(index: Record<string, unknown>): Record<string, unknown> {
  return (index.functionGraphs as Array<Record<string, unknown>>)[0];
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

it("binds project metadata to the requested project and database snapshot publication", async () => {
  ipc.invoke.mockClear();
  ipc.response = "D:/demo/metadata.yssbi";
  await expect(ProjectService.getProjectPath("project-a")).resolves.toBe(ipc.response);
  expect(ipc.invoke).toHaveBeenLastCalledWith("get_project_path", {
    projectInstanceId: "project-a",
  });

  ipc.response = { databases: {} };
  await expect(ProjectService.getDatabases("project-a", 7)).resolves.toEqual(ipc.response);
  expect(ipc.invoke).toHaveBeenLastCalledWith("get_project_databases", {
    projectInstanceId: "project-a",
    expectedPublicationRevision: 7,
  });
});

it("validates project database metadata without repairing malformed columns", async () => {
  const database = {
    id: "sales",
    engine: { dataset: {} },
    name: "Sales",
    schemaVersion: 1,
    required: false,
    loadFailed: false,
    columns: [{ name: "value", type: "Int64", physical: "Int64", semantic: null }],
    columnCount: 1,
  };
  ipc.response = { databases: { sales: database } };
  await expect(ProjectService.getDatabases("project-a", 7)).resolves.toEqual(ipc.response);
  for (const invalid of [
    { ...database, columns: [null] },
    { ...database, columnCount: 0 },
    { ...database, id: "another-database" },
  ]) {
    ipc.response = { databases: { sales: invalid } };
    await expect(ProjectService.getDatabases("project-a", 7)).rejects.toThrow();
  }
});

describe("ProjectService.getProjectIndex function editor projection parser", () => {
  beforeEach(() => {
    ipc.invoke.mockClear();
    ipc.response = projectIndex();
  });

  it("preserves the exact Rust-resolved output name and structured pin types", async () => {
    const { index } = await ProjectService.getProjectIndex("project-a");

    const functionRow = index.functionGraphs[0];
    expect(functionRow.type).toBe("function_graph");
    if (functionRow.type !== "function_graph") throw new Error("expected function row");
    expect(functionRow.functionEditorProjection).toEqual({
      functionRevision: 11,
      inputs: [
        {
          id: "sales",
          name: "Observed sales",
          dataType: { kind: "DataSeries", inner: { kind: "Scalar", inner: "Numeric" } },
        },
      ],
      outputs: [
        {
          id: "return",
          name: "Array<String>",
          dataType: { kind: "Array", inner: { kind: "Scalar", inner: "Text" } },
        },
      ],
    });
  });

  it("checks exact function signature fields while preserving Rust type names", async () => {
    const index = projectIndex();
    const signature = {
      parameters: [{ id: "opaque-id", name: "Input 名称", type_name: "Future<Opaque Rust Type>" }],
      return_type: null,
    };
    functionRow(index).functionSignature = signature;
    ipc.response = index;
    const parsed = await ProjectService.getProjectIndex("project-a");
    expect(parsed.index.functionGraphs[0].functionSignature).toEqual(signature);

    for (const invalid of [
      { ...signature, extra: true },
      { ...signature, parameters: [{ ...signature.parameters[0], extra: true }] },
      { ...signature, parameters: [{ ...signature.parameters[0], type_name: 42 }] },
    ]) {
      functionRow(index).functionSignature = invalid;
      await expect(ProjectService.getProjectIndex("project-a")).rejects.toThrow(
        "Invalid project index response",
      );
    }
  });

  it("accepts opaque event and function paths containing spaces and Unicode", async () => {
    const index = projectIndex();
    const row = functionRow(index);
    row.path = "functions/Sales Report 销售预测.yssbi-function";
    (index.eventGraphs as unknown[]).unshift({
      path: "events/每日 Sales Report.yssbi-event",
      name: "Daily report",
      type: "event_graph",
      revision: 3,
    });
    ipc.response = index;

    await expect(ProjectService.getProjectIndex("project-a")).resolves.toMatchObject({
      index: {
        eventGraphs: [{ path: "events/每日 Sales Report.yssbi-event", type: "event_graph" }],
        functionGraphs: [
          { path: "functions/Sales Report 销售预测.yssbi-function", type: "function_graph" },
        ],
      },
    });
  });

  it("uses the explicit file type while preserving an opaque path", async () => {
    const index = projectIndex();
    functionRow(index).path = "events/opaque-function-identity";
    ipc.response = index;
    await expect(ProjectService.getProjectIndex("project-a")).resolves.toMatchObject({
      index: {
        functionGraphs: [{ path: "events/opaque-function-identity", type: "function_graph" }],
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
        minds: [],
        docs: [],
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
      "missing chart path",
      (row: Record<string, unknown>) => {
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
