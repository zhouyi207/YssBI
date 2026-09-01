import { useCallback, useState } from "react";

interface DatabaseGridCell {
  row: number;
  column: number;
}

interface DatabaseGridRange extends DatabaseGridCell {
  rowCount: number;
  columnCount: number;
}

export type DatabaseGridSelection =
  | {
      type: "cells";
      activeCell: DatabaseGridCell;
      ranges: readonly DatabaseGridRange[];
    }
  | {
      type: "rows";
      rows: readonly number[];
    }
  | {
      type: "columns";
      columns: readonly number[];
    };

export function isEmptyGridSelection(selection: DatabaseGridSelection | null | undefined): boolean {
  if (!selection) return true;
  if (selection.type === "rows") return selection.rows.length === 0;
  if (selection.type === "columns") return selection.columns.length === 0;
  return false;
}

function appendValidRow(rows: number[], seen: Set<number>, row: number, rowCount: number): void {
  if (!Number.isInteger(row) || row < 0 || row >= rowCount || seen.has(row)) return;
  seen.add(row);
  rows.push(row);
}

export function selectedRowIndicesFromSelection(
  selection: DatabaseGridSelection | null | undefined,
  columnCount: number,
  rowCount: number,
): number[] {
  if (!selection || rowCount <= 0) return [];

  const rows: number[] = [];
  const seen = new Set<number>();
  if (selection.type === "rows") {
    for (const row of selection.rows) appendValidRow(rows, seen, row, rowCount);
    return rows;
  }
  if (selection.type !== "cells" || columnCount <= 0) return rows;

  for (const range of selection.ranges) {
    const isIntegerRange =
      Number.isInteger(range.row) &&
      Number.isInteger(range.column) &&
      Number.isInteger(range.rowCount) &&
      Number.isInteger(range.columnCount);
    const coversAllColumns = range.column <= 0 && range.column + range.columnCount >= columnCount;
    if (!isIntegerRange || range.rowCount <= 0 || !coversAllColumns) continue;

    const start = Math.max(0, range.row);
    const end = Math.min(rowCount, range.row + range.rowCount);
    for (let row = start; row < end; row += 1) {
      appendValidRow(rows, seen, row, rowCount);
    }
  }
  return rows;
}

export function createSelectAllSelection(
  columnCount: number,
  rowCount: number,
): DatabaseGridSelection | null {
  if (
    !Number.isInteger(columnCount) ||
    !Number.isInteger(rowCount) ||
    columnCount <= 0 ||
    rowCount <= 0
  ) {
    return null;
  }
  return {
    type: "cells",
    activeCell: { row: 0, column: 0 },
    ranges: [{ row: 0, column: 0, rowCount, columnCount }],
  };
}

interface UseSelectionParams {
  columnCount: number;
  rowCount: number;
}

export function useSelection({ columnCount, rowCount }: UseSelectionParams) {
  const [selection, setSelection] = useState<DatabaseGridSelection | null>(null);

  const selectedRowIndices = useCallback(
    () => selectedRowIndicesFromSelection(selection, columnCount, rowCount),
    [selection, columnCount, rowCount],
  );

  const selectAll = useCallback(() => {
    const nextSelection = createSelectAllSelection(columnCount, rowCount);
    if (nextSelection) setSelection(nextSelection);
  }, [columnCount, rowCount]);

  const clearSelection = useCallback(() => setSelection(null), []);

  return {
    selection,
    setSelection,
    selectedRowIndices,
    selectAll,
    clearSelection,
  };
}
