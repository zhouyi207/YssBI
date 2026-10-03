import { describe, expect, it } from "vitest";
import { getGridSelectionPrimaryCellPreview } from "./gridSelectionCellPreview";

describe("database grid cell preview", () => {
  it("previews the primary cell with its column name and absolute row number", () => {
    const columns = [{ name: "Name" }, { name: "Value" }];
    const loadedRows = [
      ["r0c0", "r0c1"],
      ["r1c0", "r1c1"],
    ];

    expect(
      getGridSelectionPrimaryCellPreview(
        {
          type: "cells",
          activeCell: { row: 1, column: 1 },
          ranges: [{ row: 0, column: 0, rowCount: 2, columnCount: 2 }],
        },
        columns,
        loadedRows,
        1000,
      ),
    ).toEqual({ rowNumber: 1002, columnName: "Value", text: "r1c1" });
    expect(
      getGridSelectionPrimaryCellPreview({ type: "rows", rows: [1] }, columns, loadedRows, 1000),
    ).toEqual({ rowNumber: 1002, columnName: "Name", text: "r1c0" });
    expect(
      getGridSelectionPrimaryCellPreview(
        { type: "columns", columns: [1] },
        columns,
        loadedRows,
        1000,
      ),
    ).toEqual({ rowNumber: 1001, columnName: "Value", text: "r0c1" });
  });

  it("clears absent or stale selections but retains the location of a selected null value", () => {
    const columns = [{ name: "Value" }];
    const loadedRows = [[null]];

    expect(getGridSelectionPrimaryCellPreview(null, columns, loadedRows, 0)).toBeNull();
    expect(
      getGridSelectionPrimaryCellPreview({ type: "rows", rows: [1] }, columns, loadedRows, 0),
    ).toBeNull();
    expect(
      getGridSelectionPrimaryCellPreview({ type: "rows", rows: [0] }, columns, loadedRows, 0),
    ).toEqual({ rowNumber: 1, columnName: "Value", text: null });
  });

  it("preserves the distinction between null, empty strings, and literal text", () => {
    const columns = [{ name: "Value" }];
    const loadedRows = [[null], [""], ["null"], ["  "]];

    const previews = loadedRows.map((_, row) =>
      getGridSelectionPrimaryCellPreview({ type: "rows", rows: [row] }, columns, loadedRows, 0),
    );

    expect(previews.map((preview) => preview?.text)).toEqual([null, "", "null", "  "]);
    expect(previews.every((preview) => preview !== null)).toBe(true);
  });
});
