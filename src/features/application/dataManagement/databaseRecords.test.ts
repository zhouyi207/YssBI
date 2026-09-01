import { describe, expect, it } from "vitest";
import type { DatabaseRecord } from "@/shared/types/domain/database";
import { normalizeDatabaseRecord, normalizeDatabases } from "./databaseRecords";

describe("normalizeDatabaseRecord", () => {
  it("derives display name from csv engine path when name is missing", () => {
    const record = normalizeDatabaseRecord("df-1", {
      id: "df-1",
      engine: { csv: { path: "C:/data/sales_report.csv" } },
    });
    expect(record.name).toBe("sales_report");
    expect(record.loadFailed).toBe(false);
  });

  it("normalizes machine load state without retaining raw load errors", () => {
    const record = normalizeDatabaseRecord("df-1", {
      id: "df-1",
      loadFailed: true,
      loadError: "sensitive backend failure",
    });

    expect(record.loadFailed).toBe(true);
    expect(record).not.toHaveProperty("loadError");
  });

  it("preserves rich metadata when incoming only supplies engine", () => {
    const existing: DatabaseRecord = {
      id: "df-1",
      name: "Kept Name",
      columns: [{ name: "a", type: "Int64" }],
      rowCount: 42,
      columnCount: 1,
      loadFailed: true,
    };
    const record = normalizeDatabaseRecord(
      "df-1",
      { id: "df-1", engine: { csv: { path: "/tmp/other.csv" } } },
      existing,
    );
    expect(record.name).toBe("other");
    expect(record.columns).toEqual(existing.columns);
    expect(record.rowCount).toBe(42);
    expect(record.loadFailed).toBe(true);
  });

  it("falls back to existing name when incoming has no name or engine", () => {
    const existing: DatabaseRecord = {
      id: "df-1",
      name: "Kept Name",
    };
    const record = normalizeDatabaseRecord("df-1", { id: "df-1" }, existing);
    expect(record.name).toBe("Kept Name");
  });
});

describe("normalizeDatabases", () => {
  it("batch-normalizes and merges against existing map", () => {
    const existing: Record<string, DatabaseRecord> = {
      "df-1": {
        id: "df-1",
        name: "Previous Name",
        rowCount: 10,
        columns: [{ name: "x", type: "Utf8" }],
      },
    };
    const result = normalizeDatabases(
      {
        "df-1": { engine: { parquet: { path: "/archive/metrics.parquet" } } },
        "df-2": { name: "New Table", rowCount: 3 },
      },
      existing,
    );
    expect(result["df-1"].name).toBe("metrics");
    expect(result["df-1"].rowCount).toBe(10);
    expect(result["df-1"].engine).toEqual({ parquet: { path: "/archive/metrics.parquet" } });
    expect(result["df-2"].name).toBe("New Table");
    expect(result["df-2"].rowCount).toBe(3);
  });
});
