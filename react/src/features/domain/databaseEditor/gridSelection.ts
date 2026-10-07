export interface DatabaseGridCellAddress {
  row: number;
  column: number;
}

export interface DatabaseGridCellRange extends DatabaseGridCellAddress {
  rowCount: number;
  columnCount: number;
}

export interface DatabaseGridSelectionModifiers {
  additive: boolean;
  extend: boolean;
}

export type DatabaseGridSelection =
  | {
      type: "cells";
      activeCell: DatabaseGridCellAddress;
      ranges: readonly DatabaseGridCellRange[];
    }
  | { type: "rows"; rows: readonly number[] }
  | { type: "columns"; columns: readonly number[] };

export function createCellRange(
  start: DatabaseGridCellAddress,
  end: DatabaseGridCellAddress,
): DatabaseGridCellRange {
  const row = Math.min(start.row, end.row);
  const column = Math.min(start.column, end.column);
  return {
    row,
    column,
    rowCount: Math.abs(start.row - end.row) + 1,
    columnCount: Math.abs(start.column - end.column) + 1,
  };
}

export function createKeyboardCellSelection(
  selection: DatabaseGridSelection | null,
  anchor: DatabaseGridCellAddress | null,
  target: DatabaseGridCellAddress,
  extend: boolean,
) {
  const nextAnchor = extend
    ? (anchor ?? (selection?.type === "cells" ? selection.activeCell : target))
    : target;
  return {
    anchor: nextAnchor,
    selection: {
      type: "cells" as const,
      activeCell: target,
      ranges: [createCellRange(nextAnchor, target)],
    },
  };
}

export function rangeContainsCell(
  range: DatabaseGridCellRange,
  row: number,
  column: number,
): boolean {
  return (
    row >= range.row &&
    row < range.row + range.rowCount &&
    column >= range.column &&
    column < range.column + range.columnCount
  );
}

export function isGridCellSelected(
  selection: DatabaseGridSelection | null,
  row: number,
  column: number,
): boolean {
  if (!selection) return false;
  if (selection.type === "columns") return selection.columns.includes(column);
  if (selection.type !== "cells") return false;
  return selection.ranges.some((range) => rangeContainsCell(range, row, column));
}

export function isGridCellActive(
  selection: DatabaseGridSelection | null,
  row: number,
  column: number,
): boolean {
  return (
    selection?.type === "cells" &&
    selection.activeCell.row === row &&
    selection.activeCell.column === column
  );
}

export function isGridColumnSelected(
  selection: DatabaseGridSelection | null,
  column: number,
): boolean {
  return selection?.type === "columns" && selection.columns.includes(column);
}

export function updateIndexSelection(
  current: readonly number[],
  index: number,
  anchor: number | null,
  modifiers: DatabaseGridSelectionModifiers,
  itemCount: number,
): number[] {
  if (!Number.isInteger(index) || index < 0 || index >= itemCount) return [...current];

  if (modifiers.extend && anchor !== null && anchor >= 0 && anchor < itemCount) {
    const start = Math.min(anchor, index);
    const end = Math.max(anchor, index);
    const range = Array.from({ length: end - start + 1 }, (_, offset) => start + offset);
    return modifiers.additive ? [...new Set([...current, ...range])] : range;
  }

  if (!modifiers.additive) return [index];
  return current.includes(index) ? current.filter((value) => value !== index) : [...current, index];
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
