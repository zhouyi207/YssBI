import { describe, expect, it } from "vitest";
import type { DatabaseRecord } from "@/shared/types/domain/database";
import { resourceKey, type ProjectResourceMeta } from "@/features/core/resource/resourceTypes";
import type { ProjectDatabaseIndexRow } from "@/shared/types/domain/project";
import type { ProjectDatabaseMetadata } from "@/services/database/databaseWireParser";
import { prepareDatabaseIndexSnapshot } from "./databaseRecords";

const row: ProjectDatabaseIndexRow = {
  id: "df-1",
  name: "Index Name",
  resourcePath: "opaque database resource",
  revision: 1,
  engine: { dataset: {} },
  schemaVersion: 1,
  required: false,
};
const key = resourceKey({ kind: "database", id: row.id });
const resource: ProjectResourceMeta = {
  id: row.id,
  kind: "database",
  name: row.name!,
  uri: key,
  revision: 1,
  exists: true,
  loaded: true,
  hasDirtyDocument: false,
  hasStaleDocument: false,
  hasConflictDocument: false,
};

describe("database index declarations", () => {
  it("uses the dataset identity when no Rust display name is available", () => {
    const record = prepareDatabaseIndexSnapshot([{ ...row, name: null }], {}, {})[row.id];
    expect(record.name).toBe("df-1");
    expect(record.loadFailed).toBe(false);
  });

  it("preserves rich cached metadata at the same resource revision", () => {
    const existing: DatabaseRecord = {
      id: "df-1",
      name: "Kept Name",
      columns: [
        {
          name: "a",
          type: "Int64",
          physical: "Int64",
          semantic: { kind: "Identifier", values: [], positiveValue: null, numeric: null },
        },
      ],
      rowCount: 42,
      columnCount: 1,
      loadFailed: true,
    };
    const record = prepareDatabaseIndexSnapshot([row], { [row.id]: existing }, { [key]: resource })[
      row.id
    ];
    expect(record.name).toBe(row.name);
    expect(record.columns).toEqual(existing.columns);
    expect(record.rowCount).toBe(42);
    expect(record.loadFailed).toBe(true);
    expect(record.columns).toBe(existing.columns);
  });

  it("uses the current index name without retaining an old display name", () => {
    const existing: DatabaseRecord = {
      id: "df-1",
      name: "Kept Name",
    };
    const record = prepareDatabaseIndexSnapshot([row], { [row.id]: existing }, {})[row.id];
    expect(record.name).toBe(row.name);
  });
});

describe("prepareDatabaseIndexSnapshot", () => {
  it("isolates fresh column metadata and index configuration from mutable callers", () => {
    const incomingRow = structuredClone(row);
    const metadata: ProjectDatabaseMetadata = {
      id: row.id,
      engine: { dataset: {} },
      schemaVersion: 1,
      required: false,
      loadFailed: false,
      columnCount: 1,
      columns: [
        {
          name: "category",
          type: "Utf8",
          physical: "Utf8",
          supportedSemanticTypes: ["Categorical", "Ordinal", "Binary", "Text", "Identifier"],
          semantic: {
            kind: "Categorical",
            values: [{ value: "a", label: "First" }],
            positiveValue: null,
            numeric: { integer: false, minimum: "0", maximum: "10" },
          },
        },
      ],
    };
    const result = prepareDatabaseIndexSnapshot([incomingRow], {}, {}, { [row.id]: metadata });
    metadata.columns[0].name = "Changed externally";
    metadata.columns[0].semantic!.values[0].label = "Changed externally";
    metadata.columns[0].semantic!.numeric!.minimum = "-10";
    metadata.columns[0].supportedSemanticTypes.length = 0;
    metadata.columns.push({
      name: "extra",
      type: "Utf8",
      physical: "Utf8",
      semantic: null,
      supportedSemanticTypes: [],
    });
    incomingRow.engine.dataset = {};
    expect(result[row.id].columns).toHaveLength(1);
    expect(result[row.id].columns![0]).toMatchObject({
      name: "category",
      supportedSemanticTypes: ["Categorical", "Ordinal", "Binary", "Text", "Identifier"],
      semantic: { values: [{ value: "a", label: "First" }], numeric: { minimum: "0" } },
    });
    expect(result[row.id].engine).not.toBe(incomingRow.engine);
  });

  it("retains unchanged column semantics while replacing changed values and removing columns", () => {
    const metadata: ProjectDatabaseMetadata = {
      id: row.id,
      engine: { dataset: {} },
      schemaVersion: 1,
      required: false,
      loadFailed: false,
      columnCount: 3,
      columns: [
        {
          name: "stable",
          type: "Int64",
          physical: "Int64",
          semantic: null,
          supportedSemanticTypes: ["Numeric", "Categorical", "Ordinal", "Binary", "Identifier"],
        },
        {
          name: "category",
          type: "Utf8",
          physical: "Utf8",
          supportedSemanticTypes: ["Categorical", "Ordinal", "Binary", "Text", "Identifier"],
          semantic: {
            kind: "Categorical",
            values: [
              { value: "a", label: "First" },
              { value: "b", label: "Second" },
            ],
            positiveValue: null,
            numeric: { integer: false, minimum: null, maximum: null },
          },
        },
        {
          name: "removed",
          type: "Utf8",
          physical: "Utf8",
          semantic: null,
          supportedSemanticTypes: [],
        },
      ],
    };
    const before = prepareDatabaseIndexSnapshot([row], {}, {}, { [row.id]: metadata });
    const repeated = prepareDatabaseIndexSnapshot(
      [structuredClone(row)],
      before,
      { [key]: resource },
      {
        [row.id]: structuredClone(metadata),
      },
    );
    expect(repeated).toBe(before);
    const changed = structuredClone(metadata);
    changed.columns.pop();
    changed.columnCount = 2;
    changed.columns[1].semantic!.values[1].label = "Renamed";
    const after = prepareDatabaseIndexSnapshot(
      [row],
      before,
      { [key]: resource },
      { [row.id]: changed },
    );
    expect(after[row.id].columns).toHaveLength(2);
    expect(after[row.id].columns![0]).toBe(before[row.id].columns![0]);
    const previousSemantic = before[row.id].columns![1].semantic!;
    const nextSemantic = after[row.id].columns![1].semantic!;
    expect(after[row.id].columns![1].supportedSemanticTypes).toBe(
      before[row.id].columns![1].supportedSemanticTypes,
    );
    expect(nextSemantic.numeric).toBe(previousSemantic.numeric);
    expect(nextSemantic.values[0]).toBe(previousSemantic.values[0]);
    expect(nextSemantic.values[1].label).toBe("Renamed");
    changed.columns[1].physical = "Int64";
    changed.columns[1].supportedSemanticTypes = [
      "Numeric",
      "Categorical",
      "Ordinal",
      "Binary",
      "Identifier",
    ];
    const converted = prepareDatabaseIndexSnapshot(
      [row],
      after,
      { [key]: resource },
      { [row.id]: changed },
    );
    expect(converted[row.id].columns![1].supportedSemanticTypes).toEqual(
      changed.columns[1].supportedSemanticTypes,
    );
    expect(converted[row.id].columns![0]).toBe(after[row.id].columns![0]);
    expect(after[row.id].columns![1].supportedSemanticTypes).toContain("Text");
    expect(previousSemantic.values[1].label).toBe("Second");
    changed.columns[1].semantic!.values[1].label = "External mutation";
    expect(nextSemantic.values[1].label).toBe("Renamed");
  });

  it("merges fresh metadata with matching cached revisions and authoritative declarations", () => {
    const existing: Record<string, DatabaseRecord> = {
      "df-1": {
        id: "df-1",
        name: "Previous Name",
        rowCount: 10,
        columns: [{ name: "x", type: "Utf8" }],
      },
    };
    const rows: ProjectDatabaseIndexRow[] = ["df-1", "df-2"].map((id, index) => ({
      id,
      name: index ? "New Table" : "Previous Name",
      resourcePath: `databases/${id}`,
      revision: 1,
      engine: { dataset: {} },
      schemaVersion: 1,
      required: false,
    }));
    const resources = Object.fromEntries(
      rows.map((row) => {
        const key = resourceKey({ kind: "database", id: row.id });
        return [
          key,
          {
            id: row.id,
            kind: "database" as const,
            name: row.name!,
            uri: key,
            revision: 1,
            exists: true,
            loaded: true,
            hasDirtyDocument: false,
            hasStaleDocument: false,
            hasConflictDocument: false,
          },
        ];
      }),
    );
    const result = prepareDatabaseIndexSnapshot(rows, existing, resources, {
      "df-1": {
        id: "df-1",
        engine: { dataset: {} },
        schemaVersion: 1,
        required: false,
        loadFailed: false,
        columns: [
          {
            name: "x",
            type: "Utf8",
            physical: "Utf8",
            semantic: null,
            supportedSemanticTypes: ["Categorical", "Ordinal", "Binary", "Text", "Identifier"],
          },
        ],
        columnCount: 1,
      },
      "df-2": {
        id: "df-2",
        engine: { dataset: {} },
        schemaVersion: 1,
        required: false,
        loadFailed: false,
        name: "Ignored metadata label",
        rowCount: 3,
        columns: [],
        columnCount: 0,
      },
    });
    expect(result["df-1"].name).toBe("Previous Name");
    expect(result["df-1"].rowCount).toBe(10);
    expect(result["df-1"].engine).toEqual({ dataset: {} });
    expect(result["df-2"].name).toBe("New Table");
    expect(result["df-2"].rowCount).toBe(3);
    expect(result["df-2"].columns).toEqual([]);
    expect(() => prepareDatabaseIndexSnapshot(rows, existing, resources, {})).toThrow(
      "Project database metadata membership does not match the index",
    );
  });
});
