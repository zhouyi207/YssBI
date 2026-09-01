import { isEmptyGridSelection, type DatabaseGridSelection } from "./useSelection";

function formatCellForPreview(value: unknown): string {
  if (value === null) return "null";
  if (value === undefined) return "";
  if (typeof value === "boolean") return value ? "true" : "false";
  if (typeof value === "object") return JSON.stringify(value);
  return String(value);
}

function getCellText(
  row: number,
  column: number,
  columnCount: number,
  rowCount: number,
  loadedRows: readonly (readonly unknown[])[],
): string {
  if (
    !Number.isInteger(row) ||
    !Number.isInteger(column) ||
    row < 0 ||
    row >= rowCount ||
    column < 0 ||
    column >= columnCount
  ) {
    return "";
  }
  const rowData = loadedRows[row];
  if (!rowData) return "";
  return formatCellForPreview(rowData[column]);
}

export function getGridSelectionPrimaryCellText(
  selection: DatabaseGridSelection | null,
  columnCount: number,
  rowCount: number,
  loadedRows: readonly (readonly unknown[])[],
): string {
  if (!selection || isEmptyGridSelection(selection)) return "";

  if (selection.type === "cells") {
    return getCellText(
      selection.activeCell.row,
      selection.activeCell.column,
      columnCount,
      rowCount,
      loadedRows,
    );
  }
  if (selection.type === "rows") {
    return getCellText(selection.rows[0], 0, columnCount, rowCount, loadedRows);
  }
  return getCellText(0, selection.columns[0], columnCount, rowCount, loadedRows);
}
