import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import type { ResourceMutationResultDto } from "@/shared/types/dto/editorMutation";
import { DatabaseService } from "./databaseService";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const projectInstanceId = "00000000-0000-0000-0000-000000000601";
const operationId = "00000000-0000-0000-0000-000000000401";
const expectedRevision = 4;
const mutation = {
  operationId,
  projectInstanceId,
  publicationRevision: 1,
  moves: [],
  deltas: [],
  projectionReplacements: [],
  projectionStatus: { status: "complete", expectedGraphPaths: [] },
  history: { canUndo: false, canRedo: false },
} satisfies ResourceMutationResultDto;

beforeEach(() => {
  vi.clearAllMocks();
});

describe("DatabaseService project lifecycle contract", () => {
  it.each([
    ["getDatabaseMeta", "get_database_meta", [projectInstanceId, "sales"], {}],
    [
      "getDatabaseRows",
      "get_database_rows",
      [projectInstanceId, "sales", 0, 50],
      { offset: 0, limit: 50 },
    ],
    ["getColumnStats", "get_column_stats", [projectInstanceId, "sales"], {}],
    ["getColumnDistribution", "get_column_distribution", [projectInstanceId, "sales"], {}],
    ["getDatasetOverview", "get_dataset_overview", [projectInstanceId, "sales"], {}],
    [
      "exportDatabase",
      "export_database",
      [projectInstanceId, "sales", "C:/sales.csv", "csv"],
      { path: "C:/sales.csv", format: "csv" },
    ],
  ] as const)("passes exact project identity through %s", async (method, command, args, extra) => {
    vi.mocked(invoke).mockResolvedValue(
      method === "getDatabaseRows" ? { rows: [], rowIds: [] } : undefined,
    );

    await (DatabaseService[method] as (...values: any[]) => Promise<unknown>)(...args);

    expect(invoke).toHaveBeenCalledWith(command, {
      projectInstanceId,
      id: "sales",
      ...extra,
    });
  });

  it("keeps external-source discovery commands project-independent", async () => {
    vi.mocked(invoke).mockResolvedValue([]);

    await DatabaseService.listSqliteTables("C:/source.sqlite");
    await DatabaseService.listSqlTables("postgres", "postgres://localhost/source");
    await DatabaseService.listExcelSheets("C:/source.xlsx");

    expect(invoke).toHaveBeenNthCalledWith(1, "list_sqlite_tables", { dbPath: "C:/source.sqlite" });
    expect(invoke).toHaveBeenNthCalledWith(2, "list_sql_tables", {
      engine: "postgres",
      connectionString: "postgres://localhost/source",
    });
    expect(invoke).toHaveBeenNthCalledWith(3, "list_excel_sheets", { filePath: "C:/source.xlsx" });
  });
});

describe("DatabaseService revisioned mutation contract", () => {
  it("passes caller project and operation identity for expected-absent imports and returns the aggregate", async () => {
    const engine = { csv: { path: "C:/sales.csv", delimiter: ",", hasHeader: true } } as const;
    const aggregate = {
      data: { id: "sales", name: "Sales", rowCount: 1, columnCount: 1, columns: [] },
      mutation,
    };
    vi.mocked(invoke).mockResolvedValue(aggregate);

    await expect(
      DatabaseService.loadDatabase(projectInstanceId, operationId, engine),
    ).resolves.toBe(aggregate);
    expect(invoke).toHaveBeenCalledWith("load_database", {
      projectInstanceId,
      operationId,
      engine,
    });
  });

  it.each([
    [
      "deleteDatabase",
      "delete_database",
      [projectInstanceId, operationId, expectedRevision, "sales"],
      {},
    ],
    [
      "renameDatabase",
      "rename_database",
      [projectInstanceId, operationId, expectedRevision, "sales", "Renamed"],
      { name: "Renamed" },
    ],
  ] as const)(
    "passes exact revision authority through %s",
    async (method, command, args, extra) => {
      const aggregate = { data: null, mutation };
      vi.mocked(invoke).mockResolvedValue(aggregate);

      await expect(
        (DatabaseService[method] as (...values: any[]) => Promise<unknown>)(...args),
      ).resolves.toBe(aggregate);
      expect(invoke).toHaveBeenCalledWith(command, {
        projectInstanceId,
        operationId,
        expectedRevision,
        id: "sales",
        ...extra,
      });
    },
  );
});
