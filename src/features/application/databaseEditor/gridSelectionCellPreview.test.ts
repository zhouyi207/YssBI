import { describe, expect, it } from "vitest";
import { getGridSelectionPrimaryCellText } from "./gridSelectionCellPreview";

describe("database grid cell preview", () => {
  it("previews the primary cell for cell, row, and column selections", () => {
    const loadedRows = [
      ["r0c0", "r0c1"],
      ["r1c0", "r1c1"],
    ];

    expect(
      getGridSelectionPrimaryCellText(
        {
          type: "cells",
          activeCell: { row: 1, column: 1 },
          ranges: [{ row: 0, column: 0, rowCount: 2, columnCount: 2 }],
        },
        2,
        2,
        loadedRows,
      ),
    ).toBe("r1c1");
    expect(getGridSelectionPrimaryCellText({ type: "rows", rows: [1] }, 2, 2, loadedRows)).toBe(
      "r1c0",
    );
    expect(
      getGridSelectionPrimaryCellText({ type: "columns", columns: [1] }, 2, 2, loadedRows),
    ).toBe("r0c1");
  });
});
