import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import type { ResourceMutationResultDto } from "@/shared/types/dto/editorMutation";
import { DatabaseService } from "./databaseService";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const projectInstanceId = "00000000-0000-0000-0000-000000000601";
const operationId = "00000000-0000-0000-0000-000000000401";
const expectedRevision = 4;
const metadata = {
  id: "sales",
  name: "Sales",
  rowCount: 1,
  columnCount: 1,
  columns: [
    {
      name: "value",
      type: "Int64",
      physical: "Int64",
      semantic: null,
      supportedSemanticTypes: ["Numeric", "Categorical", "Ordinal", "Binary", "Identifier"],
    },
  ],
};
const mutation = {
  operationId,
  projectInstanceId,
  publicationRevision: 1,
  moves: [],
  deltas: [],
  projectionReplacements: [],
  projectionStatus: { status: "complete", expectedGraphPaths: [] },
} satisfies ResourceMutationResultDto;

beforeEach(() => {
  vi.clearAllMocks();
});

describe("DatabaseService project lifecycle contract", () => {
  it("rejects invalid metadata before read or import consumers can accept it", async () => {
    vi.mocked(invoke).mockResolvedValue({ ...metadata, dataRevision: "1", id: "another-database" });
    await expect(
      DatabaseService.getDatabaseMeta(projectInstanceId, metadata.id, expectedRevision),
    ).rejects.toThrow();

    for (const invalid of [
      { ...metadata, columnCount: 2 },
      { ...metadata, columns: [{ ...metadata.columns[0], physical: undefined }] },
      { ...metadata, columns: [{ ...metadata.columns[0], semantic: undefined }] },
      { ...metadata, columns: [{ ...metadata.columns[0], semantic: { kind: "Scalar" } }] },
      { ...metadata, rowCount: -1 },
      ...[undefined, ["Int64"], ["Numeric", "Numeric"]].map((supportedSemanticTypes) => ({
        ...metadata,
        columns: [{ ...metadata.columns[0], supportedSemanticTypes }],
      })),
    ]) {
      vi.mocked(invoke).mockResolvedValue({ ...invalid, dataRevision: "1" });
      await expect(
        DatabaseService.getDatabaseMeta(projectInstanceId, metadata.id, expectedRevision),
      ).rejects.toThrow();
      vi.mocked(invoke).mockResolvedValue({ data: invalid, mutation });
      await expect(
        DatabaseService.loadDatabase(projectInstanceId, operationId, {
          csv: { path: "C:/sales.csv" },
        }),
      ).rejects.toThrow();
    }
  });

  it("validates the bounded sample projection before publishing the list", async () => {
    const sample = {
      id: "iris",
      name: "Iris",
      version: 1,
      rowCount: 150,
      columnCount: 5,
      byteSize: 2861,
    };
    vi.mocked(invoke).mockResolvedValue([sample]);
    await expect(DatabaseService.listSampleDatasets()).resolves.toEqual([sample]);
    expect(invoke).toHaveBeenLastCalledWith("list_sample_datasets");
    for (const payload of [
      [sample, sample],
      [{ ...sample, rowCount: -1 }],
      [{ ...sample, path: "private-path" }],
      [{ ...sample, version: 0 }],
    ]) {
      vi.mocked(invoke).mockResolvedValue(payload);
      await expect(DatabaseService.listSampleDatasets()).rejects.toThrow(
        "Invalid sample catalog response",
      );
    }
  });
  it("requires a lossless row-data revision on metadata reads", async () => {
    const current = { ...metadata, dataRevision: "18446744073709551615" };
    vi.mocked(invoke).mockResolvedValue(current);
    await expect(
      DatabaseService.getDatabaseMeta(projectInstanceId, "sales", expectedRevision),
    ).resolves.toEqual(current);
    for (const dataRevision of [undefined, 1, "-1", "01", "18446744073709551616"]) {
      vi.mocked(invoke).mockResolvedValue({ ...metadata, dataRevision });
      await expect(
        DatabaseService.getDatabaseMeta(projectInstanceId, "sales", expectedRevision),
      ).rejects.toThrow();
    }
  });
  it("keeps the backend row identities aligned with their page rows", async () => {
    const page = {
      rows: [
        ["9007199254740993", true, null],
        [9.5, false, "text"],
      ],
      rowIds: ["9007199254740993", "9223372036854775807"],
    };
    vi.mocked(invoke).mockResolvedValue(page);

    await expect(
      DatabaseService.getDatabaseRows(projectInstanceId, "sales", expectedRevision, 50, 2),
    ).resolves.toEqual(page);
    for (const malformed of [
      { ...page, rowIds: [Number("9007199254740993"), 3] },
      { ...page, rowIds: ["01", "3"] },
      { ...page, rowIds: ["9223372036854775808", "3"] },
      { ...page, rowIds: ["3", "3"] },
      { ...page, rowIds: ["3"] },
      { ...page, rows: [[{}], [9]] },
      { ...page, rows: [[7], [9, 10]] },
      { ...page, rows: [[Infinity], [9]] },
      { rows: [[7], [9], [10]], rowIds: ["0", "1", "2"] },
    ]) {
      vi.mocked(invoke).mockResolvedValue(malformed);
      await expect(
        DatabaseService.getDatabaseRows(projectInstanceId, "sales", expectedRevision, 50, 2),
      ).rejects.toThrow();
    }
  });

  it("validates both distribution variants before chart consumers receive them", async () => {
    const numeric = { columnName: "value", kind: "numeric", bins: [{ label: "[0, 1)", count: 2 }] };
    const categorical = {
      columnName: "category",
      kind: "string",
      categories: [{ label: "A", value: 1 }],
      otherCount: 1,
    };
    vi.mocked(invoke).mockResolvedValue([numeric, categorical]);
    await expect(
      DatabaseService.getColumnDistribution(projectInstanceId, "sales", expectedRevision),
    ).resolves.toEqual([numeric, categorical]);
    for (const invalid of [
      [{ ...numeric, bins: [{ label: "[0, 1)", count: -1 }] }],
      [{ ...categorical, categories: [{ label: "A", value: "1" }] }],
      [numeric, numeric],
      [{ ...categorical, kind: "unknown" }],
    ]) {
      vi.mocked(invoke).mockResolvedValue(invalid);
      await expect(
        DatabaseService.getColumnDistribution(projectInstanceId, "sales", expectedRevision),
      ).rejects.toThrow();
    }
  });

  it("keeps exact column values and rejects incomplete domain wire data", async () => {
    const values = ["", "001", "9007199254740993", "中文"];
    vi.mocked(invoke).mockResolvedValue(values);
    await expect(
      DatabaseService.getColumnValues(projectInstanceId, "sales", expectedRevision, "value"),
    ).resolves.toEqual(values);
    expect(invoke).toHaveBeenLastCalledWith("get_column_values", {
      projectInstanceId,
      id: "sales",
      expectedRevision,
      colName: "value",
    });
    for (const invalid of [
      [null],
      [9007199254740992],
      ["same", "same"],
      { values, truncated: true },
      Array.from({ length: 65_537 }, (_, i) => String(i)),
    ]) {
      vi.mocked(invoke).mockResolvedValue(invalid);
      await expect(
        DatabaseService.getColumnValues(projectInstanceId, "sales", expectedRevision, "value"),
      ).rejects.toThrow();
    }
  });

  it.each([
    [
      "getDatabaseMeta",
      "get_database_meta",
      [projectInstanceId, "sales", expectedRevision],
      { expectedRevision },
    ],
    [
      "getDatabaseRows",
      "get_database_rows",
      [projectInstanceId, "sales", expectedRevision, 0, 50],
      { expectedRevision, offset: 0, limit: 50 },
    ],
    [
      "getColumnDistribution",
      "get_column_distribution",
      [projectInstanceId, "sales", expectedRevision],
      { expectedRevision },
    ],
    [
      "exportDatabase",
      "export_database",
      [projectInstanceId, "sales", "C:/sales.csv", "csv"],
      { path: "C:/sales.csv", format: "csv" },
    ],
  ] as const)("passes exact project identity through %s", async (method, command, args, extra) => {
    vi.mocked(invoke).mockResolvedValue(
      method === "getDatabaseRows"
        ? { rows: [], rowIds: [] }
        : method === "getDatabaseMeta"
          ? { ...metadata, dataRevision: "1" }
          : method === "getColumnDistribution"
            ? []
            : undefined,
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
    const entries = ["  preserved name  ", "表格", "A.B"];
    vi.mocked(invoke).mockResolvedValue(entries);
    await expect(DatabaseService.listExcelSheets("C:/source.xlsx")).resolves.toEqual(entries);
    vi.mocked(invoke).mockResolvedValue(["valid", { name: "invalid" }]);
    await expect(DatabaseService.listSqliteTables("C:/source.sqlite")).rejects.toThrow();
    await expect(
      DatabaseService.listSqlTables("postgres", "postgres://localhost/source"),
    ).rejects.toThrow();
    await expect(DatabaseService.listExcelSheets("C:/source.xlsx")).rejects.toThrow();
  });
});

describe("DatabaseService revisioned mutation contract", () => {
  it("binds a sample import to the caller project, operation and exact sample version", async () => {
    const aggregate = {
      data: { ...metadata, id: "new-dataset", name: "Iris" },
      mutation,
    };
    vi.mocked(invoke).mockResolvedValue(aggregate);
    await expect(
      DatabaseService.importSampleDataset(projectInstanceId, operationId, "iris", 1),
    ).resolves.toEqual(aggregate);
    expect(invoke).toHaveBeenCalledWith("import_sample_dataset", {
      projectInstanceId,
      operationId,
      sampleId: "iris",
      version: 1,
    });
  });
  it("passes caller project and operation identity for expected-absent imports and returns the aggregate", async () => {
    const engine = { csv: { path: "C:/sales.csv", delimiter: ",", hasHeader: true } } as const;
    const aggregate = {
      data: metadata,
      mutation,
    };
    vi.mocked(invoke).mockResolvedValue(aggregate);

    await expect(
      DatabaseService.loadDatabase(projectInstanceId, operationId, engine),
    ).resolves.toEqual(aggregate);
    expect(invoke).toHaveBeenCalledWith("load_database", {
      projectInstanceId,
      operationId,
      engine,
    });
    vi.mocked(invoke).mockResolvedValue({
      ...aggregate,
      mutation: { ...mutation, deltas: [{ payload: { kind: "database", patch: null } }] },
    });
    await expect(
      DatabaseService.loadDatabase(projectInstanceId, operationId, engine),
    ).rejects.toThrow();
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
    [
      "setColumnSemantic",
      "set_column_semantic",
      [
        projectInstanceId,
        operationId,
        expectedRevision,
        "sales",
        "id",
        { kind: "Identifier", values: [], positiveValue: null, numeric: null },
      ],
      {
        colName: "id",
        semantic: { kind: "Identifier", values: [], positiveValue: null, numeric: null },
      },
    ],
  ] as const)(
    "passes exact revision authority through %s",
    async (method, command, args, extra) => {
      const aggregate = {
        data:
          method === "setColumnSemantic"
            ? { canUndo: true, canRedo: false, isModified: true, undoCount: 1, redoCount: 0 }
            : null,
        mutation,
      };
      vi.mocked(invoke).mockResolvedValue(aggregate);

      await expect(
        (DatabaseService[method] as (...values: any[]) => Promise<unknown>)(...args),
      ).resolves.toEqual(aggregate);
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
