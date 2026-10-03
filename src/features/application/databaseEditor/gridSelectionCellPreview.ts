import type { DatabaseGridSelection } from "@/features/domain/databaseEditor/gridSelection";
import type { ColumnInfo } from "@/shared/types/domain/database";

function formatCellForPreview(value: unknown): string | null {
  if (value === null) return null;
  if (value === undefined) return "";
  if (typeof value === "boolean") return value ? "true" : "false";
  if (typeof value === "object") return JSON.stringify(value);
  return String(value);
}

export function getGridSelectionPrimaryCellPreview(
  selection: DatabaseGridSelection | null,
  columns: readonly Pick<ColumnInfo, "name">[],
  loadedRows: readonly (readonly unknown[])[],
  pageStartIndex: number,
): { rowNumber: number; columnName: string; text: string | null } | null {
  if (!selection) return null;

  const row =
    selection.type === "cells"
      ? selection.activeCell.row
      : selection.type === "rows"
        ? selection.rows[0]
        : 0;
  const column =
    selection.type === "cells"
      ? selection.activeCell.column
      : selection.type === "columns"
        ? selection.columns[0]
        : 0;
  if (
    !Number.isInteger(row) ||
    !Number.isInteger(column) ||
    row < 0 ||
    row >= loadedRows.length ||
    column < 0 ||
    column >= columns.length
  ) {
    return null;
  }
  const rowData = loadedRows[row];
  if (!rowData) return null;
  return {
    rowNumber: pageStartIndex + row + 1,
    columnName: columns[column].name,
    text: formatCellForPreview(rowData[column]),
  };
}
