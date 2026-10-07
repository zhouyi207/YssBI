import { describe, expect, it } from "vitest";
import { createKeyboardCellSelection, createSelectAllSelection } from "./gridSelection";

describe("database grid keyboard selection", () => {
  it("creates select-all selection", () => {
    expect(createSelectAllSelection(3, 5)).toEqual({
      type: "cells",
      activeCell: { row: 0, column: 0 },
      ranges: [{ row: 0, column: 0, rowCount: 5, columnCount: 3 }],
    });
  });

  it("extends from the stable anchor only while Shift is held", () => {
    const initial = createKeyboardCellSelection(null, null, { row: 1, column: 1 }, false);
    expect(initial).toEqual({
      anchor: { row: 1, column: 1 },
      selection: {
        type: "cells",
        activeCell: { row: 1, column: 1 },
        ranges: [{ row: 1, column: 1, rowCount: 1, columnCount: 1 }],
      },
    });

    const extended = createKeyboardCellSelection(
      initial.selection,
      initial.anchor,
      { row: 1, column: 3 },
      true,
    );
    expect(extended).toEqual({
      anchor: { row: 1, column: 1 },
      selection: {
        type: "cells",
        activeCell: { row: 1, column: 3 },
        ranges: [{ row: 1, column: 1, rowCount: 1, columnCount: 3 }],
      },
    });

    expect(
      createKeyboardCellSelection(extended.selection, extended.anchor, { row: 2, column: 3 }, false)
        .anchor,
    ).toEqual({ row: 2, column: 3 });
  });
});
